use std::collections::{HashMap, HashSet};

use crate::project::compiler::Compiler;
use crate::render::{BlendMode, RenderList};

pub type NodeId = u64;

/// The editable grade workflow, owned by the runtime's [`EditSession`].
#[derive(Clone, Debug)]
pub struct WorkFlow {
    version: u64,
    // We stored the connection relation ship inside th node,
    // so we do not need edge vector.
    nodes: Vec<Node>,
    output: Source,
    allocator: NodeIdAlloc,
    /// Derived DAG cache, the single source of truth is (nodes, output)
    graph: WorkFlowGraph,
}

#[derive(Clone, Debug)]
pub struct NodeIdAlloc {
    /// Next available node id
    counter: NodeId,
}

impl NodeIdAlloc {
    fn new() -> Self {
        Self { counter: 0 }
    }
    fn alloc(&mut self) -> NodeId {
        self.counter += 1;
        self.counter - 1
    }
}

/// Thd source of node.
/// We don't need any edge struct since all node has only one output.
/// We can indicates edges by store the node's source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// From the input of current workflow
    Input,
    /// Comes from another node
    Node(NodeId),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub node_id: NodeId,
    pub attr: NodeKind,
    pub pos: (f32, f32),
}

/// We allowed a node without any input.
/// Maybe user just haven't connect anything on it after create it.
#[derive(Clone, Debug)]
pub enum NodeKind {
    /// A noremal process node with 1 input and 1 output
    Normal {
        source: Option<Source>,
        brightness: f32,
        saturation: f32,
    },
    /// A layer merge node with n input and 1 output
    Merge {
        /// Ordered bottom-to-top.
        sources: Vec<Source>,
        blend_mode: BlendMode,
    },
    /// A generator node with 0 input and 1 output
    Generator { mode: GeneratorMode },
}

#[derive(Clone, Debug)]
pub enum GeneratorMode {
    RGBA8 { color: [u8; 4] },
}

impl NodeKind {
    fn default_normal() -> Self {
        Self::Normal {
            source: None,
            brightness: 1.0,
            saturation: 1.0,
        }
    }
    fn default_merge() -> Self {
        Self::Merge {
            sources: Vec::new(),
            blend_mode: BlendMode::Top,
        }
    }
    /// Return if diff legal
    fn apply_diff(&mut self, diff: NodeKindDiff) -> bool {
        match (self, diff) {
            (NodeKind::Normal { brightness, .. }, NodeKindDiff::Brightness(new)) => {
                *brightness = new;
            }
            (NodeKind::Normal { saturation, .. }, NodeKindDiff::Saturation(new)) => {
                *saturation = new;
            }
            (NodeKind::Merge { blend_mode, .. }, NodeKindDiff::BlendMode(new)) => {
                *blend_mode = new;
            }
            _ => return false,
        }
        true
    }
}

impl Default for WorkFlow {
    fn default() -> Self {
        let mut allocator = NodeIdAlloc::new();
        let node_id = allocator.alloc();
        let nodes = vec![Node {
            node_id,
            attr: NodeKind::Normal {
                source: Some(Source::Input),
                brightness: 1.0,
                saturation: 1.0,
            },
            pos: (1.0, 1.0), // TODO: Calculate more accurate default position
        }];
        let graph = WorkFlowGraph::build(&nodes).expect("default workflow must be a DAG");
        Self {
            version: 0,
            nodes,
            output: Source::Node(node_id),
            allocator,
            graph,
        }
    }
}

impl WorkFlow {
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.node_id == id)
    }

    pub fn output(&self) -> Source {
        self.output
    }

    pub(super) fn graph(&self) -> &WorkFlowGraph {
        &self.graph
    }

    pub fn compile(&self) -> anyhow::Result<RenderList> {
        // TODO: Determine whether enable optimize by outer context
        Compiler::new(true).compile(self)
    }

    /// Return whether need to rerender
    pub fn apply_diff(&mut self, diff: WorkFlowDiff) -> bool {
        if diff.version != self.version {
            info!("Received outdated workflow diff, ignore it");
            return false;
        }
        let applied = match diff.payload {
            WorkFlowDiffPayload::NodeAttr(node_id, attr_diff) => {
                let Some(node) = self.nodes.iter_mut().find(|node| node.node_id == node_id) else {
                    warn!("Cannot apply diff: node {node_id} does not exist");
                    return false;
                };
                if !node.attr.apply_diff(attr_diff) {
                    warn!("Cannot apply diff: diff does not match node {node_id} kind");
                    return false;
                }
                true
            }
            WorkFlowDiffPayload::NodePos(node_id, new_pos) => {
                let Some(node) = self.nodes.iter_mut().find(|node| node.node_id == node_id) else {
                    warn!("Cannot apply diff: node {node_id} does not exist");
                    return false;
                };
                node.pos = new_pos;
                true
            }
        };
        if applied {
            self.version += 1;
        }
        applied
    }

    /// Check if workflow finish edit.
    /// The unfinished state is a legal state so we should verify if it finished before compile.
    /// This is called by compiler.
    /// If cannot be compiled return None, else return Some(useful subset of graph)
    pub fn prepare_compile(&self) -> Option<HashSet<usize>> {
        let reachable = self.graph.reachable();
        let Source::Node(output) = self.output else {
            // input -> output
            return Some(HashSet::new());
        };
        let output = self.graph.index_of(output)?;
        if !reachable.contains(&output) {
            // output is not reachable
            return None;
        }
        Some(self.graph.ancestors_of(output))
    }
}

/// An owned DAG cache derived from `NodeKind` sources.
#[derive(Clone, Debug)]
pub(super) struct WorkFlowGraph {
    index_of: HashMap<NodeId, usize>,
    predecessors: Vec<Vec<usize>>,
    successors: Vec<Vec<usize>>,
    topological_order: Vec<usize>,
    /// Stores nodes which's input is satisfied and reachable from graph's inputs
    reachable: HashSet<usize>,
}

impl WorkFlowGraph {
    /// Build graph cache and verify whole workflow
    /// TODO: Maybe we can do less work inside this build
    fn build(nodes: &[Node]) -> anyhow::Result<Self> {
        // build index_of
        let mut index_of = HashMap::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            let old = index_of.insert(node.node_id, index);
            anyhow::ensure!(old.is_none(), "duplicated node id: {}", node.node_id);
        }

        // build predecessors and successors
        let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        let mut successors: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        for (to, node) in nodes.iter().enumerate() {
            let mut add_source = |source: Source| -> anyhow::Result<()> {
                let Source::Node(source_id) = source else {
                    return Ok(());
                };
                let from = index_of.get(&source_id).copied().ok_or_else(|| {
                    anyhow::anyhow!(
                        "node {} references missing source node {source_id}",
                        node.node_id
                    )
                })?;
                if !predecessors[to].contains(&from) {
                    predecessors[to].push(from);
                    successors[from].push(to);
                }
                Ok(())
            };

            match &node.attr {
                NodeKind::Normal {
                    source: Some(source),
                    ..
                } => add_source(*source)?,
                NodeKind::Merge { sources, .. } => {
                    for source in sources {
                        add_source(*source)?;
                    }
                }
                NodeKind::Normal { source: None, .. } | NodeKind::Generator { .. } => {}
            }
        }

        // build topogogical order
        let mut indegree: Vec<usize> = predecessors.iter().map(Vec::len).collect();
        let mut ready: Vec<usize> = (0..indegree.len())
            .filter(|&index| indegree[index] == 0)
            .collect();
        let mut order = Vec::with_capacity(indegree.len());
        while let Some(index) = ready.pop() {
            order.push(index);
            for &next in &successors[index] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    ready.push(next);
                }
            }
        }
        anyhow::ensure!(order.len() == indegree.len(), "workflow contains a cycle");

        // build reachable
        let mut reachable = HashSet::new();
        for &index in &order {
            let source_reachable = |source: &Source| match source {
                Source::Input => true,
                Source::Node(id) => index_of
                    .get(id)
                    .is_some_and(|index| reachable.contains(index)),
            };
            let node_reachable = match &nodes[index].attr {
                NodeKind::Normal {
                    source: Some(source),
                    ..
                } => source_reachable(source),
                NodeKind::Normal { source: None, .. } => false,
                NodeKind::Merge { sources, .. } => {
                    sources.len() >= 2 && sources.iter().all(source_reachable)
                }
                NodeKind::Generator { .. } => true,
            };
            if node_reachable {
                reachable.insert(index);
            }
        }

        Ok(Self {
            index_of,
            predecessors,
            successors,
            topological_order: order,
            reachable,
        })
    }

    fn reachable(&self) -> &HashSet<usize> {
        &self.reachable
    }

    fn ancestors_of(&self, output: usize) -> HashSet<usize> {
        let mut ancestors = HashSet::from([output]);
        let mut stack = vec![output];
        while let Some(index) = stack.pop() {
            for &previous in &self.predecessors[index] {
                if ancestors.insert(previous) {
                    stack.push(previous);
                }
            }
        }
        ancestors
    }

    pub(super) fn topological_order(&self) -> &[usize] {
        &self.topological_order
    }

    pub(super) fn predecessors(&self, index: usize) -> &[usize] {
        &self.predecessors[index]
    }

    pub(super) fn successors(&self, index: usize) -> &[usize] {
        &self.successors[index]
    }

    pub(super) fn index_of(&self, id: NodeId) -> Option<usize> {
        self.index_of.get(&id).copied()
    }
}

/// Store a diff for incremental update
/// Only support update one node in a time
#[derive(Clone)]
pub struct WorkFlowDiff {
    pub version: u64,
    pub payload: WorkFlowDiffPayload,
}

#[derive(Clone)]
pub enum WorkFlowDiffPayload {
    NodeAttr(NodeId, NodeKindDiff),
    NodePos(NodeId, (f32, f32)),
}

#[derive(Clone)]
pub enum NodeKindDiff {
    Brightness(f32),
    Saturation(f32),
    BlendMode(BlendMode),
}

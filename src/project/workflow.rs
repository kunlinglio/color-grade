use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::project::compiler::Compiler;
use crate::render::{BlendMode, RenderList};

pub type NodeId = u64;

/// The editable grade workflow, owned by the runtime's [`EditSession`].
#[derive(Clone, Debug)]
pub struct Workflow {
    // We stored the connection relation ship inside th node,
    // so we do not need edge vector.
    nodes: Vec<Node>,
    output: Option<Source>,
    allocator: NodeIdAlloc,
    /// Derived DAG cache, the single source of truth is (nodes, output)
    graph: WorkflowGraph,
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
    pub kind: NodeKind,
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
        /// `None` is an unconnected slot: it is ignored when compiling, but a Merge
        /// needs at least two connected inputs to be considered finished.
        sources: Vec<Option<Source>>,
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
    fn apply_diff(&mut self, diff: &NodeKindDiff) -> anyhow::Result<()> {
        match (self, diff) {
            (NodeKind::Normal { brightness, .. }, NodeKindDiff::Brightness(new)) => {
                *brightness = *new;
            }
            (NodeKind::Normal { saturation, .. }, NodeKindDiff::Saturation(new)) => {
                *saturation = *new;
            }
            (NodeKind::Merge { blend_mode, .. }, NodeKindDiff::BlendMode(new)) => {
                *blend_mode = *new;
            }
            _ => anyhow::bail!("Unmatch NodeKindDiff"),
        }
        Ok(())
    }

    fn verify_diff(&self, diff: &NodeKindDiff) -> anyhow::Result<()> {
        match (self, diff) {
            (NodeKind::Normal { .. }, NodeKindDiff::Brightness(..)) => {}
            (NodeKind::Normal { .. }, NodeKindDiff::Saturation(..)) => {}
            (NodeKind::Merge { .. }, NodeKindDiff::BlendMode(..)) => {}
            _ => anyhow::bail!("Unmatch NodeKindDiff"),
        }
        Ok(())
    }
}

impl Default for Workflow {
    fn default() -> Self {
        let mut allocator = NodeIdAlloc::new();
        let node_id = allocator.alloc();
        let nodes = vec![Node {
            node_id,
            kind: NodeKind::Normal {
                source: Some(Source::Input),
                brightness: 1.0,
                saturation: 1.0,
            },
            pos: (1.0, 1.0), // TODO: Calculate more accurate default position
        }];
        let graph = WorkflowGraph::build(&nodes).expect("default workflow must be a DAG");
        Self {
            nodes,
            output: Some(Source::Node(node_id)),
            allocator,
            graph,
        }
    }
}

impl Workflow {
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.node_id == id)
    }

    pub fn output(&self) -> Option<Source> {
        self.output
    }

    pub(super) fn graph(&self) -> &WorkflowGraph {
        &self.graph
    }

    pub fn compile(&self, optimize: bool) -> anyhow::Result<RenderList> {
        Compiler::new(optimize).compile(self)
    }

    pub fn verify_diff(&self, diff: &WorkflowDiff) -> anyhow::Result<()> {
        match &diff.payload {
            WorkflowDiffPayload::NodeAttr(node_id, attr_diff) => {
                let Some(node) = self.nodes.iter().find(|node| node.node_id == *node_id) else {
                    anyhow::bail!("Cannot apply diff: node {node_id} does not exist");
                };
                node.kind.verify_diff(attr_diff).with_context(|| {
                    format!("Cannot apply diff: diff does not match node {node_id} kind")
                })?;
            }
            WorkflowDiffPayload::NodePos(node_id, ..) => {
                let Some(_) = self.nodes.iter().find(|node| node.node_id == *node_id) else {
                    anyhow::bail!("Cannot apply diff: node {node_id} does not exist");
                };
            }
        };
        Ok(())
    }

    /// Return whether need to rerender or the diff is invalid
    pub fn apply_diff(&mut self, diff: &WorkflowDiff) -> anyhow::Result<bool> {
        let need_render = match &diff.payload {
            WorkflowDiffPayload::NodeAttr(node_id, attr_diff) => {
                let Some(node) = self.nodes.iter_mut().find(|node| node.node_id == *node_id) else {
                    anyhow::bail!("Cannot apply diff: node {node_id} does not exist");
                };
                node.kind.apply_diff(attr_diff).with_context(|| {
                    format!("Cannot apply diff: diff does not match node {node_id} kind")
                })?;
                true
            }
            WorkflowDiffPayload::NodePos(node_id, new_pos) => {
                let Some(node) = self.nodes.iter_mut().find(|node| node.node_id == *node_id) else {
                    anyhow::bail!("Cannot apply diff: node {node_id} does not exist");
                };
                node.pos = *new_pos;
                false
            }
        };
        Ok(need_render)
    }

    /// Check if workflow finish edit.
    /// The unfinished state is a legal state so we should verify if it finished before compile.
    /// This is called by compiler.
    /// If cannot be compiled return None, else return Some(useful subset of graph)
    pub fn prepare_compile(&self) -> Option<HashSet<usize>> {
        let reachable = self.graph.reachable();
        let Some(output) = self.output else {
            // output is not connected
            return None;
        };
        let Source::Node(output) = output else {
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
pub(super) struct WorkflowGraph {
    index_of: HashMap<NodeId, usize>,
    predecessors: Vec<Vec<usize>>,
    successors: Vec<Vec<usize>>,
    topological_order: Vec<usize>,
    /// Stores nodes whose input is satisfied and reachable from graph's inputs
    reachable: HashSet<usize>,
}

impl WorkflowGraph {
    /// Builds the derived DAG cache and validates the workflow.
    ///
    /// Must be called after every structural edit; value edits (params, position)
    /// leave the cache untouched.
    /// Eager rebuild is a trade-off:
    /// it rejects invalid topologies (duplicate ids, dangling sources, cycles) at edit time
    /// and lets the much more frequent `compile` reuse the cached order/index.
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

            match &node.kind {
                NodeKind::Normal {
                    source: Some(source),
                    ..
                } => add_source(*source)?,
                NodeKind::Merge { sources, .. } => {
                    for source in sources {
                        let Some(source) = *source else {
                            continue;
                        };
                        add_source(source)?;
                    }
                }
                NodeKind::Normal { source: None, .. } | NodeKind::Generator { .. } => {}
            }
        }

        // build topological order
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
            let node_reachable = match &nodes[index].kind {
                NodeKind::Normal {
                    source: Some(source),
                    ..
                } => source_reachable(source),
                NodeKind::Normal { source: None, .. } => false,
                NodeKind::Merge { sources, .. } => {
                    sources.iter().flatten().count() >= 2
                        && sources.iter().flatten().all(source_reachable)
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
#[derive(Clone, Copy)]
pub struct WorkflowDiff {
    pub payload: WorkflowDiffPayload,
}

#[derive(Clone, Copy)]
pub enum WorkflowDiffPayload {
    NodeAttr(NodeId, NodeKindDiff),
    NodePos(NodeId, (f32, f32)),
}

#[derive(Clone, Copy)]
pub enum NodeKindDiff {
    Brightness(f32),
    Saturation(f32),
    BlendMode(BlendMode),
}

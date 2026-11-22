use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::project::compiler::Compiler;
use crate::render::{BlendMode, RenderList};

pub type NodeId = u64;

/// The editable grade workflow, owned by the runtime's [`EditSession`].
#[derive(Clone, Debug)]
pub struct Workflow {
    // We stored the connection relation ship inside the node,
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
        /// Ordered Top-bottom.
        /// `None` is an unconnected slot: it is ignored when compiling, but a Merge
        /// needs at least two connected inputs to be considered finished.
        sources: Vec<Option<Source>>,
        kind: MergeNodeKind,
    },
    /// A generator node with 0 input and 1 output
    Generator { mode: GeneratorMode },
}

#[derive(Clone, Debug, Copy, Default)]
pub enum MergeNodeKind {
    /// Normal layer blend, just like ps.
    LayerBlend(BlendMode),
    /// Layer mixer, mix all changes from different diverse directly.
    #[default]
    Mixer,
}

#[derive(Clone, Debug)]
pub enum GeneratorMode {
    RGBA8 { color: [u8; 4] },
}

impl NodeKind {
    fn default_normal(source: Option<Source>) -> Self {
        Self::Normal {
            source,
            brightness: 1.0,
            saturation: 1.0,
        }
    }
    fn default_merge(sources: Vec<Option<Source>>) -> Self {
        Self::Merge {
            sources,
            kind: MergeNodeKind::default(),
        }
    }
    /// Return if diff legal
    fn apply_diff(&mut self, diff: &NodeAttrDiff) -> anyhow::Result<()> {
        match (self, diff) {
            (NodeKind::Normal { brightness, .. }, NodeAttrDiff::Brightness(new)) => {
                *brightness = *new;
            }
            (NodeKind::Normal { saturation, .. }, NodeAttrDiff::Saturation(new)) => {
                *saturation = *new;
            }
            (NodeKind::Merge { kind, .. }, NodeAttrDiff::MergeMode(new)) => {
                *kind = *new;
            }
            _ => anyhow::bail!("Unmatch NodeKindDiff"),
        }
        Ok(())
    }

    fn verify_diff(&self, diff: &NodeAttrDiff) -> anyhow::Result<()> {
        match (self, diff) {
            (NodeKind::Normal { .. }, NodeAttrDiff::Brightness(..)) => {}
            (NodeKind::Normal { .. }, NodeAttrDiff::Saturation(..)) => {}
            (NodeKind::Merge { .. }, NodeAttrDiff::MergeMode(..)) => {}
            _ => anyhow::bail!("Unmatch NodeKindDiff"),
        }
        Ok(())
    }

    fn source_slots_mut(&mut self) -> &mut [Option<Source>] {
        match self {
            NodeKind::Normal { source, .. } => std::slice::from_mut(source),
            NodeKind::Merge { sources, .. } => sources,
            NodeKind::Generator { .. } => &mut [],
        }
    }

    fn sources_mut(&mut self) -> impl Iterator<Item = &mut Source> {
        self.source_slots_mut().iter_mut().flatten()
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
            pos: (40., 40.),
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
        // The derived graph keeps the id -> index map, so this is O(1).
        self.graph.index_of(id).map(|index| &self.nodes[index])
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        let index = self.graph.index_of(id)?;
        self.nodes.get_mut(index)
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
                let Some(node) = self.get_node(*node_id) else {
                    anyhow::bail!("Cannot apply diff: node {node_id} does not exist");
                };
                node.kind.verify_diff(attr_diff).with_context(|| {
                    format!("Cannot apply diff: diff does not match node {node_id} kind")
                })?;
            }
            WorkflowDiffPayload::NodePos(node_id, ..) => {
                anyhow::ensure!(
                    self.get_node(*node_id).is_some(),
                    "Cannot apply diff: node {node_id} does not exist"
                );
            }
            WorkflowDiffPayload::Topology(change) => {
                self.try_topology(change)?;
            }
        };
        Ok(())
    }

    /// Return whether need to rerender or the diff is invalid
    pub fn apply_diff(&mut self, diff: &WorkflowDiff) -> anyhow::Result<bool> {
        let need_render = match &diff.payload {
            WorkflowDiffPayload::NodeAttr(node_id, attr_diff) => {
                let Some(node) = self.node_mut(*node_id) else {
                    anyhow::bail!("Cannot apply diff: node {node_id} does not exist");
                };
                node.kind.apply_diff(attr_diff).with_context(|| {
                    format!("Cannot apply diff: diff does not match node {node_id} kind")
                })?;
                true
            }
            WorkflowDiffPayload::NodePos(node_id, new_pos) => {
                let Some(node) = self.node_mut(*node_id) else {
                    anyhow::bail!("Cannot apply diff: node {node_id} does not exist");
                };
                node.pos = *new_pos;
                false
            }
            WorkflowDiffPayload::Topology(change) => {
                *self = self.try_topology(change)?;
                true
            }
        };
        Ok(need_render)
    }

    /// The single mutation path for topology edits.
    /// It works on a clone, rebuilds the derived graph and returns the result.
    fn try_topology(&self, change: &WorkflowTopoChange) -> anyhow::Result<Workflow> {
        let mut copy = self.clone();
        copy.apply_topology_in_place(*change)?;
        copy.graph = WorkflowGraph::build(&copy.nodes)?;
        Ok(copy)
    }

    fn apply_topology_in_place(&mut self, change: WorkflowTopoChange) -> anyhow::Result<()> {
        match change {
            WorkflowTopoChange::AddSerialNode { from, pos } => self.add_serial_node(from, pos),
            WorkflowTopoChange::AddParreleNode {
                from,
                pos,
                node_width,
            } => self.add_parallel_node(from, pos, node_width),
            WorkflowTopoChange::DeleteNode { id } => self.delete_node(id),
            WorkflowTopoChange::AddEdge { from, to } => self.add_edge(from, to),
            WorkflowTopoChange::DeleteEdge { from, to } => self.delete_edge(from, to),
            WorkflowTopoChange::ReconnectEdge {
                old_from,
                old_to,
                new_from,
                new_to,
            } => self.reconnect_edge(old_from, old_to, new_from, new_to),
        }
    }

    fn add_serial_node(&mut self, from: NodeId, pos: (f32, f32)) -> anyhow::Result<()> {
        let from_index = self
            .graph
            .index_of(from)
            .with_context(|| format!("Cannot insert after node {from}: node does not exist"))?;
        let successors = self.graph.successors(from_index).to_vec();
        let new_id = self.allocator.alloc();

        for successor in successors {
            for source in self.nodes[successor].kind.sources_mut() {
                if *source == Source::Node(from) {
                    *source = Source::Node(new_id);
                }
            }
        }
        if self.output == Some(Source::Node(from)) {
            self.output = Some(Source::Node(new_id));
        }

        self.nodes.push(Node {
            node_id: new_id,
            kind: NodeKind::default_normal(Some(Source::Node(from))),
            pos,
        });
        Ok(())
    }

    fn add_parallel_node(
        &mut self,
        from: NodeId,
        pos: (f32, f32),
        node_width: f32,
    ) -> anyhow::Result<()> {
        let from_index = self.graph.index_of(from).with_context(|| {
            format!("Cannot add a parallel node to {from}: node does not exist")
        })?;
        let (inherited, from_pos) = match &self.nodes[from_index].kind {
            NodeKind::Normal { source, .. } => {
                let Some(source) = source else {
                    anyhow::bail!(
                        "Cannot add a parallel node to {from}: node has no input to share"
                    )
                };
                (*source, self.nodes[from_index].pos)
            }
            _ => anyhow::bail!(
                "Cannot add a parallel node to {from}: only normal nodes have a single input to share"
            ),
        };
        let successors = self.graph.successors(from_index).to_vec();
        let new_id = self.allocator.alloc();

        let existing_merge = match successors.as_slice() {
            [successor] if matches!(self.nodes[*successor].kind, NodeKind::Merge { .. }) => {
                Some(*successor)
            }
            _ => None,
        };

        if let Some(successor) = existing_merge {
            let NodeKind::Merge { sources, .. } = &mut self.nodes[successor].kind else {
                unreachable!("successor kind is checked above")
            };
            sources.push(Some(Source::Node(new_id)));
        } else {
            // A fan-out (or a plain successor) cannot take a second input, so
            // merge both branches before they reach the old successors again.
            let merge_id = self.allocator.alloc();
            for successor in successors {
                for source in self.nodes[successor].kind.sources_mut() {
                    if *source == Source::Node(from) {
                        *source = Source::Node(merge_id);
                    }
                }
            }
            if self.output == Some(Source::Node(from)) {
                self.output = Some(Source::Node(merge_id));
            }
            self.nodes.push(Node {
                node_id: merge_id,
                kind: NodeKind::default_merge(vec![
                    Some(Source::Node(from)),
                    Some(Source::Node(new_id)),
                ]),
                pos: (
                    (from_pos.0 + pos.0) / 2.0 + 1.2 * node_width,
                    (from_pos.1 + pos.1) / 2.0,
                ),
            });
        }

        self.nodes.push(Node {
            node_id: new_id,
            kind: NodeKind::default_normal(Some(inherited)),
            pos,
        });
        Ok(())
    }

    fn delete_node(&mut self, id: NodeId) -> anyhow::Result<()> {
        let index = self
            .graph
            .index_of(id)
            .with_context(|| format!("Cannot delete node {id}: node does not exist"))?;
        // Only try to heal normal node
        let heal = match &self.nodes[index].kind {
            NodeKind::Normal { source, .. } => source.filter(|source| *source != Source::Node(id)),
            _ => None,
        };

        for node in &mut self.nodes {
            for slot in node.kind.source_slots_mut() {
                if *slot == Some(Source::Node(id)) {
                    *slot = heal;
                }
            }
        }
        if self.output == Some(Source::Node(id)) {
            self.output = heal;
        }

        self.nodes.remove(index);
        Ok(())
    }

    fn source_slot_mut(
        &mut self,
        to: NodeId,
        port: usize,
        allow_append: bool,
    ) -> anyhow::Result<&mut Option<Source>> {
        let Some(node) = self.node_mut(to) else {
            anyhow::bail!("node {to} does not exist");
        };
        match &mut node.kind {
            NodeKind::Normal { source, .. } => {
                anyhow::ensure!(port == 0, "node {to} only has input port 0");
                Ok(source)
            }
            NodeKind::Merge { sources, .. } => {
                if port == sources.len() && allow_append {
                    sources.push(None);
                }
                anyhow::ensure!(port < sources.len(), "node {to} has no input port {port}");
                Ok(&mut sources[port])
            }
            NodeKind::Generator { .. } => anyhow::bail!("generator node {to} has no input port"),
        }
    }

    fn add_edge(&mut self, from: NodeId, to: (NodeId, usize)) -> anyhow::Result<()> {
        let (to_id, port) = to;
        anyhow::ensure!(
            self.graph.index_of(from).is_some(),
            "Cannot connect edge: source node {from} does not exist"
        );
        let slot = self
            .source_slot_mut(to_id, port, true)
            .with_context(|| format!("Cannot connect edge to node {to_id} port {port}"))?;
        *slot = Some(Source::Node(from));
        Ok(())
    }

    fn delete_edge(&mut self, from: NodeId, to: (NodeId, usize)) -> anyhow::Result<()> {
        let (to_id, port) = to;
        let slot = self
            .source_slot_mut(to_id, port, false)
            .with_context(|| format!("Cannot disconnect edge from node {to_id} port {port}"))?;
        anyhow::ensure!(
            *slot == Some(Source::Node(from)),
            "Cannot disconnect edge: edge from {from} to {to_id} port {port} does not exist"
        );
        *slot = None;
        Ok(())
    }

    fn reconnect_edge(
        &mut self,
        old_from: NodeId,
        old_to: (NodeId, usize),
        new_from: NodeId,
        new_to: (NodeId, usize),
    ) -> anyhow::Result<()> {
        if (old_from, old_to) == (new_from, new_to) {
            return Ok(());
        }
        let (old_to_id, old_port) = old_to;
        let (new_to_id, new_port) = new_to;

        // Validate the whole diff before touching the graph.
        {
            let old_slot = self
                .source_slot_mut(old_to_id, old_port, false)
                .with_context(|| {
                    format!("Cannot reconnect edge from node {old_to_id} port {old_port}")
                })?;
            anyhow::ensure!(
                *old_slot == Some(Source::Node(old_from)),
                "Cannot reconnect edge: edge from {old_from} to {old_to_id} port {old_port} does not exist"
            );
        }
        anyhow::ensure!(
            self.graph.index_of(new_from).is_some(),
            "Cannot reconnect edge: source node {new_from} does not exist"
        );

        if old_to == new_to {
            // Same port, new source: overwrite in place.
            let slot = self.source_slot_mut(new_to_id, new_port, false)?;
            *slot = Some(Source::Node(new_from));
        } else {
            let slot = self
                .source_slot_mut(old_to_id, old_port, false)
                .expect("old port was validated above");
            *slot = None;

            let slot = self
                .source_slot_mut(new_to_id, new_port, true)
                .with_context(|| {
                    format!("Cannot reconnect edge to node {new_to_id} port {new_port}")
                })?;
            *slot = Some(Source::Node(new_from));
        }
        Ok(())
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
    /// Nodes reading directly from the workflow input. The input is not a node,
    /// so its edges are not part of `predecessors`; they are recorded here.
    reads_input: Vec<bool>,
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
        let mut reads_input = vec![false; nodes.len()];
        for (to, node) in nodes.iter().enumerate() {
            let mut add_source = |source: Source| -> anyhow::Result<()> {
                let Source::Node(source_id) = source else {
                    // `Source::Input` is an edge as well, it just has no node on
                    // the source side, so it cannot live in `predecessors`.
                    reads_input[to] = true;
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
            reads_input,
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

    /// Ancestors of the node at `index`, including the node itself.
    /// Returns (Ancestor node indexes, input is ancestor)
    fn ancestors_of_index(&self, index: usize) -> (HashSet<usize>, bool) {
        let ancestors: HashSet<usize> = self.ancestors_of(index);
        let include_input = ancestors.iter().any(|&index| self.reads_input[index]);
        (ancestors, include_input)
    }

    /// Ancestors of `source`, including the source itself.
    /// Returns (Ancestor node indexes, input is ancestor)
    pub(super) fn ancestors_of_source(&self, source: Source) -> (HashSet<usize>, bool) {
        match source {
            Source::Input => (HashSet::new(), true),
            Source::Node(id) => {
                let index = self.index_of(id).expect("Source node must exist");
                self.ancestors_of_index(index)
            }
        }
    }

    /// Nearest common ancestor of `branches`, used to pick the base of a mixer.
    ///
    /// Returns `Ok(None)` when the workflow input is the nearest common ancestor.
    /// The nearest common ancestor is a common ancestor that is not a strict
    /// ancestor of any other common ancestor. It is unique for tree-like
    /// workflows; ambiguous topologies are reported as an error.
    pub(super) fn nearest_common_ancestor(
        &self,
        branches: &[Source],
    ) -> anyhow::Result<Option<usize>> {
        let mut sources = branches.iter();
        let Some(first) = sources.next() else {
            anyhow::bail!("cannot find the ancestor of nothing");
        };

        let (mut common, mut input_common) = self.ancestors_of_source(*first);
        for branch in sources {
            let (ancestors, branch_reads_input) = self.ancestors_of_source(*branch);
            common.retain(|ancestor| ancestors.contains(ancestor));
            input_common &= branch_reads_input;
        }
        anyhow::ensure!(
            !common.is_empty() || input_common,
            "branches have no common ancestor"
        );

        let candidates: Vec<usize> = common.into_iter().collect();
        // Each candidate's own ancestors, including itself.
        let ancestor_sets: Vec<HashSet<usize>> = candidates
            .iter()
            .map(|&index| self.ancestors_of_index(index).0)
            .collect();
        // A common ancestor is "nearest" when it is not a strict ancestor of
        // another common ancestor; that leaves the deepest nodes of the
        // common ancestor set.
        let mut nearest: Vec<usize> = Vec::new();
        for (index, &candidate) in candidates.iter().enumerate() {
            let mut is_ancestor_of_other = false;
            for (other, ancestors) in ancestor_sets.iter().enumerate() {
                if other != index && ancestors.contains(&candidate) {
                    is_ancestor_of_other = true;
                    break;
                }
            }
            if !is_ancestor_of_other {
                nearest.push(candidate);
            }
        }

        match nearest.as_slice() {
            // Only the workflow input is common to every branch.
            [] => Ok(None),
            [ancestor] => Ok(Some(*ancestor)),
            _ => {
                anyhow::bail!("branches have multiple nearest common ancestors: {nearest:?}")
            }
        }
    }

    pub(super) fn topological_order(&self) -> &[usize] {
        &self.topological_order
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
    NodeAttr(NodeId, NodeAttrDiff),
    NodePos(NodeId, (f32, f32)),
    Topology(WorkflowTopoChange),
}

#[derive(Clone, Copy)]
pub enum NodeAttrDiff {
    Brightness(f32),
    Saturation(f32),
    MergeMode(MergeNodeKind),
}

#[derive(Clone, Copy)]
pub enum WorkflowTopoChange {
    /// Insert node between current node and the successor of current node.
    /// This will automatically connect edges.
    /// Only Normal node is allowed
    AddSerialNode {
        from: NodeId,
        pos: (f32, f32),
    },
    /// Insert node in parrele with current node.
    /// e.g.
    /// Before:
    ///     A --> Current --> C
    /// After:
    ///     A --> Current --> C
    ///       |->   New   ->|
    /// This will automatically connect edges.
    /// Only Normal node is allowed
    AddParreleNode {
        from: NodeId,
        pos: (f32, f32),
        node_width: f32,
    },
    DeleteNode {
        id: NodeId,
    },
    /// Connect `from` to the input port `to.1` of node `to.0`.
    /// A port is `(node, index)`: the same source may occupy several merge ports.
    /// On a merge, `to.1 == source_count` appends a new port.
    AddEdge {
        from: NodeId,
        to: (NodeId, usize),
    },
    /// Disconnect input port `to.1` of node `to.0`; it must currently read `from`.
    DeleteEdge {
        from: NodeId,
        to: (NodeId, usize),
    },
    /// Move `old_from -> old_to` to `new_from -> new_to`, endpoints are `(node, port)`.
    ReconnectEdge {
        old_from: NodeId,
        old_to: (NodeId, usize),
        new_from: NodeId,
        new_to: (NodeId, usize),
    },
}

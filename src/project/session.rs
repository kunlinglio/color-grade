use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};

use crate::render::{BlendMode, Frame, Slot};
use crate::render::{Op, RenderList};

/// A opened or edited media inside a project.
pub struct Session {
    workflow: WorkFlow,
    decoded: Arc<Frame>,
}

impl Session {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let img = image::ImageReader::open(&path)?
            .with_guessed_format()?
            .decode()?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let decoded_frame = Arc::new(Frame::new(width as usize, height as usize, rgba.to_vec()));
        Ok(Session {
            workflow: WorkFlow::default(),
            decoded: decoded_frame,
        })
    }

    pub fn gen_render_list(&self) -> anyhow::Result<RenderList> {
        self.workflow.compile()
    }

    pub fn get_decoded(&self) -> Arc<Frame> {
        self.decoded.clone()
    }

    /// Return whether need to trigger rerender
    pub fn update(&mut self, diff: WorkFlowDiff) -> bool {
        self.workflow.apply_diff(diff)
    }

    pub fn get_workflow(&self) -> WorkFlow {
        //TODO: Optimize this clone
        self.workflow.clone()
    }
}

pub type NodeId = u64;

/// The editable grade workflow, owned by the runtime's [`EditSession`].
#[derive(Clone, Debug)]
pub struct WorkFlow {
    pub version: u64,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    // Two special edge, only record which nodes it connect to
    pub input_to: Vec<NodeId>,
    pub output_from: NodeId,
}

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

#[derive(Clone, Debug)]
pub struct Node {
    pub node_id: NodeId,
    pub attr: NodeAttr,
    pub pos: (f32, f32),
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}

#[derive(Clone, Debug)]
pub enum NodeAttr {
    Normal { brightness: f32, saturation: f32 },
    Merge { blend_mode: BlendMode },
}

impl NodeAttr {
    fn default_normal() -> Self {
        Self::Normal {
            brightness: 1.0,
            saturation: 1.0,
        }
    }
    fn default_merge() -> Self {
        Self::Merge {
            blend_mode: BlendMode::Top,
        }
    }
    fn apply_diff(&mut self, diff: NodeAttrDiff) {
        match diff {
            NodeAttrDiff::Brightness(new) => {
                let NodeAttr::Normal {
                    brightness,
                    saturation: _,
                } = self
                else {
                    panic!("Received unmatched node attr diff");
                };
                *brightness = new;
            }
            NodeAttrDiff::Saturation(new) => {
                let NodeAttr::Normal {
                    brightness: _,
                    saturation,
                } = self
                else {
                    panic!("Received unmatched node attr diff");
                };
                *saturation = new;
            }
            NodeAttrDiff::BlendMode(new) => {
                let NodeAttr::Merge { blend_mode } = self else {
                    panic!("Received unmatched node attr diff");
                };
                *blend_mode = new;
            }
        }
    }
}

impl Default for WorkFlow {
    fn default() -> Self {
        let mut allocator = NodeIdAlloc::new();
        let node_id = allocator.alloc();
        WorkFlow {
            version: 0,
            nodes: vec![Node {
                node_id,
                attr: NodeAttr::default_normal(),
                pos: (1.0, 1.0), // TODO: Calculate more accurate default position
            }],
            edges: vec![],
            input_to: vec![node_id],
            output_from: node_id,
        }
    }
}

impl WorkFlow {
    fn compile(&self) -> anyhow::Result<RenderList> {
        WorkFlowCompiler::new().compile(self)
    }
    /// Return whether need to rerender
    fn apply_diff(&mut self, diff: WorkFlowDiff) -> bool {
        if diff.version != self.version {
            info!("Received outdated workflow diff, ignore it");
            return false;
        }
        match diff.payload {
            WorkFlowDiffPayload::NodeAttr(node_id, attr_diff) => {
                self.nodes
                    .iter_mut()
                    .find(|node| node.node_id == node_id)
                    .unwrap()
                    .attr
                    .apply_diff(attr_diff);
            }
            WorkFlowDiffPayload::NodePos(node_id, new_pos) => {
                self.nodes
                    .iter_mut()
                    .find(|node| node.node_id == node_id)
                    .unwrap()
                    .pos = new_pos
            }
        }
        self.version += 1;
        true
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
    NodeAttr(NodeId, NodeAttrDiff),
    NodePos(NodeId, (f32, f32)),
}

#[derive(Clone)]
pub enum NodeAttrDiff {
    Brightness(f32),
    Saturation(f32),
    BlendMode(BlendMode),
}

/// A index-based view of the workflow graph.
struct WorkFlowGraph<'a> {
    workflow: &'a WorkFlow,
    predecessors: Vec<Vec<usize>>,
    successors: Vec<Vec<usize>>,
    inputs: Vec<usize>,
    output: usize,
}

impl<'a> WorkFlowGraph<'a> {
    fn build(workflow: &'a WorkFlow) -> anyhow::Result<Self> {
        let mut index_of = HashMap::with_capacity(workflow.nodes.len());
        for (index, node) in workflow.nodes.iter().enumerate() {
            anyhow::ensure!(
                index_of.insert(node.node_id, index).is_none(),
                "duplicated node id: {}",
                node.node_id
            );
        }

        let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); workflow.nodes.len()];
        let mut successors: Vec<Vec<usize>> = vec![Vec::new(); workflow.nodes.len()];
        for (i, edge) in workflow.edges.iter().enumerate() {
            let from = index_of.get(&edge.from).copied().ok_or_else(|| {
                anyhow::anyhow!("edge #{i} references missing node {}", edge.from)
            })?;
            let to = index_of
                .get(&edge.to)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("edge #{i} references missing node {}", edge.to))?;
            successors[from].push(to);
            predecessors[to].push(from);
        }

        let resolve = |id: NodeId| {
            index_of
                .get(&id)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("workflow references missing node {id}"))
        };
        let inputs = workflow
            .input_to
            .iter()
            .map(|&id| resolve(id))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let output = resolve(workflow.output_from)?;

        Ok(Self {
            workflow,
            predecessors,
            successors,
            inputs,
            output,
        })
    }

    /// Kahn's algorithm
    fn topological_order(&self) -> anyhow::Result<Vec<usize>> {
        let mut indegree: Vec<usize> = self.predecessors.iter().map(Vec::len).collect();
        let mut ready: VecDeque<usize> = (0..indegree.len())
            .filter(|&index| indegree[index] == 0)
            .collect();
        let mut order = Vec::with_capacity(indegree.len());
        while let Some(index) = ready.pop_front() {
            order.push(index);
            for &next in &self.successors[index] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    ready.push_back(next);
                }
            }
        }
        // Nodes that never became ready are part of a cycle.
        anyhow::ensure!(order.len() == indegree.len(), "workflow contains a cycle");
        Ok(order)
    }

    fn node(&self, index: usize) -> &'a Node {
        &self.workflow.nodes[index]
    }

    fn predecessors_of(&self, index: usize) -> &[usize] {
        &self.predecessors[index]
    }

    fn successors_of(&self, index: usize) -> &[usize] {
        &self.successors[index]
    }

    fn inputs(&self) -> &[usize] {
        &self.inputs
    }

    fn output(&self) -> usize {
        self.output
    }
    fn reachable_from_inputs(&self) -> HashSet<usize> {
        let mut seen = HashSet::new();
        let mut stack = Vec::new();
        for &index in &self.inputs {
            if !seen.contains(&index) {
                seen.insert(index);
                stack.push(index);
            }
        }
        while let Some(index) = stack.pop() {
            for &next in &self.successors[index] {
                if !seen.contains(&next) {
                    seen.insert(next);
                    stack.push(next);
                }
            }
        }
        seen
    }
    fn ancestors_of_output(&self) -> HashSet<usize> {
        let mut seen = HashSet::new();
        seen.insert(self.output);
        let mut stack = vec![self.output];
        while let Some(index) = stack.pop() {
            for &prev in &self.predecessors[index] {
                if !seen.contains(&prev) {
                    seen.insert(prev);
                    stack.push(prev);
                }
            }
        }
        seen
    }
}

struct WorkFlowCompiler {
    allocator: SlotAllocator,
    ops: Vec<Op>,
    /// The slots to be process
    /// node index -> input slots
    workset: HashMap<usize, Vec<Slot>>,
    reachable: HashSet<usize>,
    output_slot: Option<Slot>,
}

struct SlotAllocator {
    /// Reference count for all slots
    using_slots: Vec<usize>,
}

impl SlotAllocator {
    fn new() -> Self {
        Self {
            using_slots: vec![],
        }
    }
    fn slot_count(&self) -> usize {
        self.using_slots.len()
    }
    fn dup(&mut self, slot: Slot) -> Slot {
        let index = slot.id() as usize;
        assert!(
            self.using_slots.get(index).copied().unwrap_or(0usize) > 0usize,
            "duplicating an unused slot"
        );
        self.using_slots[index] += 1;
        Slot::new(index as u32)
    }
    fn alloc(&mut self) -> Slot {
        let index = match self.using_slots.iter().position(|&using| using == 0) {
            Some(i) => i,
            None => {
                self.using_slots.push(0);
                self.using_slots.len() - 1
            }
        };
        self.using_slots[index] += 1;
        Slot::new(index as u32)
    }
    fn free(&mut self, slot: Slot) {
        let index = slot.id() as usize;
        assert!(
            self.using_slots.get(index).copied().unwrap_or(0usize) > 0usize,
            "freeing an unused slot"
        );
        self.using_slots[index] -= 1;
    }
}

impl WorkFlowCompiler {
    fn new() -> Self {
        Self {
            allocator: SlotAllocator::new(),
            ops: vec![],
            workset: HashMap::new(),
            reachable: HashSet::new(),
            output_slot: None,
        }
    }

    fn compile(mut self, workflow: &WorkFlow) -> anyhow::Result<RenderList> {
        let graph = WorkFlowGraph::build(workflow)?;
        self.reachable = graph.ancestors_of_output();
        if self.reachable.len() == 0 {
            bail!("The graph is not connective")
        }
        // Prelude
        let input = {
            let input = self.allocator.alloc();
            for input_to in graph.inputs() {
                if !self.reachable.contains(input_to) {
                    continue;
                }
                self.workset
                    .entry(*input_to)
                    .or_default()
                    .push(self.allocator.dup(input));
            }
            self.allocator.free(input);
            input
        };
        // Body
        for index in graph.topological_order()? {
            if !self.reachable.contains(&index) {
                continue;
            }
            self.compile_node(&graph, index)?;
        }
        // Epilogue
        let output = {
            let result_slot = self.output_slot.expect("Should find output");
            self.allocator.free(result_slot);
            result_slot
        };
        let result = RenderList {
            input,
            output,
            slot_count: self.allocator.slot_count(),
            ops: self.ops,
        };
        result
            .verify()
            .context("compiled to an illegal render list")?;
        Ok(result)
    }

    fn compile_node(&mut self, graph: &WorkFlowGraph, index: usize) -> anyhow::Result<()> {
        let operands = self
            .workset
            .remove(&index)
            .with_context(|| format!("Cannot find input for node {index}"))?;
        match &graph.node(index).attr {
            NodeAttr::Normal {
                brightness,
                saturation,
            } => {
                anyhow::ensure!(
                    operands.len() == 1,
                    "The normal node {} has more than one input {}",
                    index,
                    operands.len()
                );
                let output = if *brightness != 1.0 && *saturation != 1.0 {
                    let medium = self.allocator.alloc();
                    let output = self.allocator.alloc();
                    self.ops.push(Op::Brightness {
                        from: operands[0],
                        to: medium,
                        brightness: *brightness,
                    });
                    self.ops.push(Op::Saturation {
                        from: medium,
                        to: output,
                        saturation: *saturation,
                    });
                    self.allocator.free(medium);
                    output
                } else if *brightness != 1.0 {
                    let output = self.allocator.alloc();
                    self.ops.push(Op::Brightness {
                        from: operands[0],
                        to: output,
                        brightness: *brightness,
                    });
                    output
                } else if *saturation != 1.0 {
                    let output = self.allocator.alloc();
                    self.ops.push(Op::Saturation {
                        from: operands[0],
                        to: output,
                        saturation: *saturation,
                    });
                    output
                } else {
                    self.allocator.dup(operands[0])
                };
                for next in graph.successors_of(index) {
                    if !self.reachable.contains(next) {
                        continue;
                    }
                    self.workset
                        .entry(*next)
                        .or_default()
                        .push(self.allocator.dup(output))
                }
                if graph.output() == index {
                    self.output_slot = Some(self.allocator.dup(output));
                }
                self.allocator.free(operands[0]);
                self.allocator.free(output);
            }
            NodeAttr::Merge { blend_mode } => {
                anyhow::ensure!(
                    operands.len() == 2,
                    "The merge node {} has input {} not equal to 2",
                    index,
                    operands.len()
                );
                let output = self.allocator.alloc();
                // FIXME: the sequence of operands is not reliable.
                self.ops.push(Op::Merge {
                    fg: operands[0],
                    bg: operands[1],
                    to: output,
                    mode: *blend_mode,
                });
                for next in graph.successors_of(index) {
                    if !self.reachable.contains(next) {
                        continue;
                    }
                    self.workset
                        .entry(*next)
                        .or_default()
                        .push(self.allocator.dup(output))
                }
                if graph.output() == index {
                    self.output_slot = Some(self.allocator.dup(output));
                }
                self.allocator.free(operands[0]);
                self.allocator.free(operands[1]);
                self.allocator.free(output);
            }
        };
        Ok(())
    }
}

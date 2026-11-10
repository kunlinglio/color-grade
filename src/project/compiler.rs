use std::collections::{HashMap, HashSet};
use std::fmt;

use anyhow::Context;

use super::session::{NodeAttr, WorkFlow};
use crate::render::{BlendMode, Op, PointwiseOp, RenderList, Slot};

pub(super) struct Compiler;

impl Compiler {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn compile(&self, workflow: &WorkFlow) -> anyhow::Result<RenderList> {
        // Stage 1: Build graph and validate workflow
        let workflow_graph = WorkFlowGraph::build(workflow)?;
        // TODO: Add more validation for workflow graph

        // Stage 2: Lower workflow to IR
        let ir = Lowerer::new(&workflow_graph).lower()?;
        ir.verify()?;

        // Stage 3: Optimize pass
        // TODO

        // Stage 4: Schedule instructions
        let sequence = ir.schedule()?;

        // Stage 5: Materialize IR with scheduled sequence.
        //      Do slot allocation ("Reg allocation") here.
        Materializer::new(&ir, sequence).materialize()
    }
}

/// An index-based, validated view of the editable workflow graph.
/// Use index of nodes rather than NodeId
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
            let old = index_of.insert(node.node_id, index);
            anyhow::ensure!(old.is_none(), "duplicated node id: {}", node.node_id);
        }

        let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); workflow.nodes.len()];
        let mut successors: Vec<Vec<usize>> = vec![Vec::new(); workflow.nodes.len()];
        for (edge_index, edge) in workflow.edges.iter().enumerate() {
            let from = index_of.get(&edge.from).copied().ok_or_else(|| {
                anyhow::anyhow!("edge #{edge_index} references missing node {}", edge.from)
            })?;
            let to = index_of.get(&edge.to).copied().ok_or_else(|| {
                anyhow::anyhow!("edge #{edge_index} references missing node {}", edge.to)
            })?;
            successors[from].push(to);
            predecessors[to].push(from);
        }

        let resolve = |id| {
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

    /// Kahn's algorithm with a LIFO ready set
    fn topological_order(&self) -> anyhow::Result<Vec<usize>> {
        let mut indegree: Vec<usize> = self.predecessors.iter().map(Vec::len).collect();
        let mut ready: Vec<usize> = (0..indegree.len())
            .filter(|&index| indegree[index] == 0)
            .collect();
        let mut order = Vec::with_capacity(indegree.len());
        while let Some(index) = ready.pop() {
            order.push(index);
            for &next in &self.successors[index] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    ready.push(next);
                }
            }
        }
        anyhow::ensure!(order.len() == indegree.len(), "workflow contains a cycle");
        Ok(order)
    }

    fn ancestors_of_output(&self) -> HashSet<usize> {
        let mut seen = HashSet::new();
        seen.insert(self.output);
        let mut stack = vec![self.output];
        while let Some(index) = stack.pop() {
            for &previous in &self.predecessors[index] {
                if seen.insert(previous) {
                    stack.push(previous);
                }
            }
        }
        seen
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ValueId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct InstructionId(usize);

#[derive(Debug)]
struct Value {
    producer: Option<InstructionId>,
    consumers: Vec<InstructionId>,
}

#[derive(Debug)]
struct IRInstruction {
    operation: IROperation,
    inputs: Vec<ValueId>,
    output: ValueId,
}

#[derive(Debug)]
enum IROperation {
    Pointwise(Vec<PointwiseOp>),
    Merge(BlendMode),
}

#[derive(Debug)]
struct IR {
    values: Vec<Value>,
    instructions: Vec<IRInstruction>,
    input: ValueId,
    output: ValueId,
}

impl fmt::Display for IR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ir(%{}) {{", self.input.0)?;

        for instruction in &self.instructions {
            write!(f, "  %{} = ", instruction.output.0)?;
            match &instruction.operation {
                IROperation::Pointwise(operations) => {
                    write!(f, "pointwise [")?;
                    for (operation_index, operation) in operations.iter().enumerate() {
                        if operation_index != 0 {
                            write!(f, ", ")?;
                        }
                        match operation {
                            PointwiseOp::Brightness(value) => {
                                write!(f, "brightness({value:?})")?;
                            }
                            PointwiseOp::Saturation(value) => {
                                write!(f, "saturation({value:?})")?;
                            }
                        }
                    }
                    write!(f, "]")?;
                }
                IROperation::Merge(mode) => match mode {
                    BlendMode::Top => write!(f, "merge.top")?,
                },
            }

            for (input_index, input) in instruction.inputs.iter().enumerate() {
                if input_index == 0 {
                    write!(f, " ")?;
                } else {
                    write!(f, ", ")?;
                }
                write!(f, "%{}", input.0)?;
            }
            writeln!(f)?;
        }

        writeln!(f, "  ret %{}", self.output.0)?;
        write!(f, "}}")
    }
}

impl IR {
    fn new() -> Self {
        let input = ValueId(0);
        Self {
            values: vec![Value {
                producer: None,
                consumers: Vec::new(),
            }],
            instructions: Vec::new(),
            input,
            output: input,
        }
    }

    fn append(&mut self, operation: IROperation, inputs: Vec<ValueId>) -> ValueId {
        let instruction_id = InstructionId(self.instructions.len());
        let output = ValueId(self.values.len());
        self.values.push(Value {
            producer: Some(instruction_id),
            consumers: Vec::new(),
        });
        for &input in &inputs {
            self.values[input.0].consumers.push(instruction_id);
        }
        self.instructions.push(IRInstruction {
            operation,
            inputs,
            output,
        });
        output
    }

    fn verify(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.input.0 < self.values.len(), "IR input is out of range");
        anyhow::ensure!(
            self.output.0 < self.values.len(),
            "IR output is out of range"
        );

        for (index, instruction) in self.instructions.iter().enumerate() {
            let id = InstructionId(index);
            anyhow::ensure!(
                self.values[instruction.output.0].producer == Some(id),
                "instruction {index} does not define its declared output"
            );
            for input in &instruction.inputs {
                anyhow::ensure!(
                    input.0 < self.values.len(),
                    "IR input value is out of range"
                );
                anyhow::ensure!(
                    self.values[input.0].consumers.contains(&id),
                    "instruction {index} is missing from value {} consumers",
                    input.0
                );
            }
        }
        Ok(())
    }

    /// Use kahn LIFO to schedule instructions
    fn schedule(&self) -> anyhow::Result<Vec<InstructionId>> {
        let mut indegree = vec![0usize; self.instructions.len()];
        for (index, instruction) in self.instructions.iter().enumerate() {
            let mut producers = HashSet::new();
            for input in &instruction.inputs {
                if let Some(producer) = self.values[input.0].producer {
                    producers.insert(producer);
                }
            }
            indegree[index] = producers.len();
        }

        let mut ready: Vec<InstructionId> = self
            .instructions
            .iter()
            .enumerate()
            .filter(|(index, _)| indegree[*index] == 0)
            .map(|(index, _)| InstructionId(index))
            .collect();
        let mut res = Vec::with_capacity(self.instructions.len());

        while let Some(instruction_id) = ready.pop() {
            res.push(instruction_id);
            let output = self.instructions[instruction_id.0].output;
            let mut consumers = HashSet::new();
            for &consumer in &self.values[output.0].consumers {
                consumers.insert(consumer);
            }
            for consumer in consumers {
                indegree[consumer.0] -= 1;
                if indegree[consumer.0] == 0 {
                    ready.push(consumer);
                }
            }
        }

        anyhow::ensure!(
            res.len() == self.instructions.len(),
            "instruction IR contains a cycle"
        );
        Ok(res)
    }
}

struct Lowerer<'graph> {
    graph: &'graph WorkFlowGraph<'graph>,
    ir: IR,
    workset: HashMap<usize, Vec<ValueId>>,
    reachable: HashSet<usize>,
}

impl<'graph> Lowerer<'graph> {
    fn new(graph: &'graph WorkFlowGraph<'graph>) -> Self {
        let reachable = graph.ancestors_of_output();
        Self {
            graph,
            ir: IR::new(),
            workset: HashMap::new(),
            reachable,
        }
    }

    fn lower(mut self) -> anyhow::Result<IR> {
        for &input_node in &self.graph.inputs {
            if self.reachable.contains(&input_node) {
                self.workset
                    .entry(input_node)
                    .or_default()
                    .push(self.ir.input);
            }
        }

        for node_index in self.graph.topological_order()? {
            if self.reachable.contains(&node_index) {
                self.lower_node(node_index)?;
            }
        }
        Ok(self.ir)
    }

    fn lower_node(&mut self, node_index: usize) -> anyhow::Result<()> {
        let operands = self
            .workset
            .remove(&node_index)
            .with_context(|| format!("cannot find input for node {node_index}"))?;

        let output = match &self.graph.workflow.nodes[node_index].attr {
            NodeAttr::Normal {
                brightness,
                saturation,
            } => {
                anyhow::ensure!(
                    operands.len() == 1,
                    "normal node {node_index} has {} inputs instead of one",
                    operands.len()
                );
                let mut current = operands[0];
                if *brightness != 1.0 {
                    current = self.ir.append(
                        IROperation::Pointwise(vec![PointwiseOp::Brightness(*brightness)]),
                        vec![current],
                    );
                }
                if *saturation != 1.0 {
                    current = self.ir.append(
                        IROperation::Pointwise(vec![PointwiseOp::Saturation(*saturation)]),
                        vec![current],
                    );
                }
                current
            }
            NodeAttr::Merge { blend_mode } => {
                anyhow::ensure!(
                    operands.len() == 2,
                    "merge node {node_index} has {} inputs instead of two",
                    operands.len()
                );
                // FIXME: This is a workaround, we need to support more accurate port.
                self.ir.append(IROperation::Merge(*blend_mode), operands)
            }
        };

        for &successor in &self.graph.successors[node_index] {
            if self.reachable.contains(&successor) {
                self.workset.entry(successor).or_default().push(output);
            }
        }
        if self.graph.output == node_index {
            self.ir.output = output;
        }
        Ok(())
    }
}

struct SlotAllocator {
    using_slots: Vec<usize>,
}

impl SlotAllocator {
    fn new() -> Self {
        Self {
            using_slots: Vec::new(),
        }
    }

    fn slot_count(&self) -> usize {
        self.using_slots.len()
    }

    fn alloc(&mut self) -> Slot {
        let index = match self.using_slots.iter().position(|&uses| uses == 0) {
            Some(index) => index,
            None => {
                self.using_slots.push(0);
                self.using_slots.len() - 1
            }
        };
        self.using_slots[index] += 1;
        Slot::new(index as u32)
    }

    fn dup(&mut self, slot: Slot) -> Slot {
        let index = slot.id() as usize;
        assert!(self.using_slots[index] > 0, "duplicating an unused slot");
        self.using_slots[index] += 1;
        slot
    }

    fn free(&mut self, slot: Slot) {
        let index = slot.id() as usize;
        assert!(self.using_slots[index] > 0, "freeing an unused slot");
        self.using_slots[index] -= 1;
    }

    fn is_own(&self, slot: Slot) -> bool {
        self.using_slots[slot.id() as usize] == 1
    }
}

struct Materializer<'ir> {
    ir: &'ir IR,
    sequence: Vec<InstructionId>,
    allocator: SlotAllocator,
    /// Value -> Slot
    value_slots: Vec<Option<Slot>>,
    /// Reference count for Values in IR
    use_counts: Vec<usize>,
    ops: Vec<Op>,
}

impl<'ir> Materializer<'ir> {
    fn new(ir: &'ir IR, sequence: Vec<InstructionId>) -> Self {
        let mut use_counts = vec![0usize; ir.values.len()];
        for &instruction_id in &sequence {
            for input in &ir.instructions[instruction_id.0].inputs {
                use_counts[input.0] += 1;
            }
        }
        // Keep the program output alive for RenderList's epilogue.
        use_counts[ir.output.0] += 1;

        Self {
            ir,
            sequence,
            allocator: SlotAllocator::new(),
            value_slots: vec![None; ir.values.len()],
            use_counts,
            ops: Vec::new(),
        }
    }

    fn materialize(mut self) -> anyhow::Result<RenderList> {
        let input_slot = self.allocator.alloc();
        self.bind_value(self.ir.input, input_slot)?;

        for instruction_id in self.sequence.clone() {
            let instruction = &self.ir.instructions[instruction_id.0];
            let input_slots = instruction
                .inputs
                .iter()
                .map(|value| {
                    self.value_slots[value.0]
                        .with_context(|| format!("value {} has no materialized slot", value.0))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            let output_slot = match &instruction.operation {
                IROperation::Pointwise(operations) => {
                    anyhow::ensure!(input_slots.len() == 1, "pointwise op is not unary");
                    let input = input_slots[0];
                    let output = if self.allocator.is_own(input) {
                        self.allocator.dup(input)
                    } else {
                        self.allocator.alloc()
                    };
                    self.ops.push(Op::Pointwise {
                        from: input,
                        to: output,
                        operations: operations.clone(),
                    });
                    output
                }
                IROperation::Merge(mode) => {
                    anyhow::ensure!(input_slots.len() == 2, "merge op is not binary");
                    let output = self.allocator.alloc();
                    self.ops.push(Op::Merge {
                        fg: input_slots[0],
                        bg: input_slots[1],
                        to: output,
                        mode: *mode,
                    });
                    output
                }
            };

            for input in input_slots {
                self.allocator.free(input);
            }
            self.bind_value(instruction.output, output_slot)?;
        }

        let output_slot = self.value_slots[self.ir.output.0].context("IR output has no slot")?;
        let result = RenderList {
            input: input_slot,
            output: output_slot,
            slot_count: self.allocator.slot_count(),
            ops: self.ops,
        };
        result
            .verify()
            .with_context(|| format!("compiled to an illegal render list {result:?}"))?;
        Ok(result)
    }

    fn bind_value(&mut self, value: ValueId, slot: Slot) -> anyhow::Result<()> {
        let uses = self.use_counts[value.0];
        anyhow::ensure!(uses > 0, "materialized value {} has no uses", value.0);
        anyhow::ensure!(self.value_slots[value.0].is_none(), "value bound twice");
        for _ in 1..uses {
            self.allocator.dup(slot);
        }
        self.value_slots[value.0] = Some(slot);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::session::{Edge, Node, NodeAttr};
    use crate::render::BlendMode;

    fn normal(node_id: u64, brightness: f32, saturation: f32) -> Node {
        Node {
            node_id,
            attr: NodeAttr::Normal {
                brightness,
                saturation,
            },
            pos: (0.0, 0.0),
        }
    }

    fn pointwise_chain(node_count: usize) -> WorkFlow {
        let nodes = (0..node_count)
            .map(|index| normal(index as u64, 1.01 + index as f32 / 100.0, 0.75))
            .collect();
        let edges = (0..node_count - 1)
            .map(|index| Edge {
                from: index as u64,
                to: index as u64 + 1,
            })
            .collect();
        WorkFlow {
            version: 0,
            nodes,
            edges,
            input_to: vec![0],
            output_from: node_count as u64 - 1,
        }
    }

    fn two_branch_workflow() -> WorkFlow {
        let mut nodes = (0..18)
            .map(|index| normal(index, 1.01 + index as f32 / 100.0, 0.75))
            .collect::<Vec<_>>();
        nodes.push(Node {
            node_id: 18,
            attr: NodeAttr::Merge {
                blend_mode: BlendMode::Top,
            },
            pos: (0.0, 0.0),
        });
        let mut edges = Vec::new();
        for branch_start in [0, 9] {
            for index in branch_start..branch_start + 8 {
                edges.push(Edge {
                    from: index,
                    to: index + 1,
                });
            }
            edges.push(Edge {
                from: branch_start + 8,
                to: 18,
            });
        }
        WorkFlow {
            version: 0,
            nodes,
            edges,
            input_to: vec![0, 9],
            output_from: 18,
        }
    }

    fn lower(workflow: &WorkFlow) -> IR {
        let graph = WorkFlowGraph::build(workflow).unwrap();
        Lowerer::new(&graph).lower().unwrap()
    }

    #[test]
    fn prints_ir_as_a_function() {
        assert_eq!(
            format!("{}", lower(&WorkFlow::default())),
            "ir(%0) {\n  ret %0\n}"
        );

        let workflow = WorkFlow {
            version: 0,
            nodes: vec![normal(0, 1.25, 1.0), normal(1, 1.0, 0.75)],
            edges: vec![Edge { from: 0, to: 1 }],
            input_to: vec![0],
            output_from: 1,
        };
        assert_eq!(
            format!("{}", lower(&workflow)),
            concat!(
                "ir(%0) {\n",
                "  %1 = pointwise [brightness(1.25)] %0\n",
                "  %2 = pointwise [saturation(0.75)] %1\n",
                "  ret %2\n",
                "}",
            )
        );

        let workflow = WorkFlow {
            version: 0,
            nodes: vec![
                normal(0, 1.25, 1.0),
                normal(1, 1.0, 0.75),
                Node {
                    node_id: 2,
                    attr: NodeAttr::Merge {
                        blend_mode: BlendMode::Top,
                    },
                    pos: (0.0, 0.0),
                },
            ],
            edges: vec![Edge { from: 0, to: 2 }, Edge { from: 1, to: 2 }],
            input_to: vec![0, 1],
            output_from: 2,
        };
        assert_eq!(
            format!("{}", lower(&workflow)),
            concat!(
                "ir(%0) {\n",
                "  %1 = pointwise [saturation(0.75)] %0\n",
                "  %2 = pointwise [brightness(1.25)] %0\n",
                "  %3 = merge.top %1, %2\n",
                "  ret %3\n",
                "}",
            )
        );
    }

    #[test]
    fn identity_workflow_materializes_no_ops() {
        let list = Compiler::new().compile(&WorkFlow::default()).unwrap();
        assert!(list.ops.is_empty());
        assert_eq!(list.slot_count, 1);
        assert_eq!(list.input, list.output);
    }

    #[test]
    fn pointwise_ops_remain_separate_without_optimization() {
        let workflow = WorkFlow {
            version: 0,
            nodes: vec![normal(0, 1.25, 1.0), normal(1, 1.0, 0.75)],
            edges: vec![Edge { from: 0, to: 1 }],
            input_to: vec![0],
            output_from: 1,
        };

        let list = Compiler::new().compile(&workflow).unwrap();
        assert_eq!(list.slot_count, 1);
        assert_eq!(list.ops.len(), 2);
        assert!(matches!(
            &list.ops[0],
            Op::Pointwise { operations, .. }
                if operations == &vec![PointwiseOp::Brightness(1.25)]
        ));
        assert!(matches!(
            &list.ops[1],
            Op::Pointwise { operations, .. }
                if operations == &vec![PointwiseOp::Saturation(0.75)]
        ));
    }

    #[test]
    fn graph_ir_compiles_pointwise_and_branching_workflows() {
        let workflow = pointwise_chain(20);
        let list = Compiler::new().compile(&workflow).unwrap();
        assert_eq!(list.ops.len(), 40);
        assert_eq!(list.slot_count, 1);

        let workflow = two_branch_workflow();
        let list = Compiler::new().compile(&workflow).unwrap();
        assert_eq!(list.ops.len(), 37);
        assert_eq!(list.slot_count, 3);
    }

    #[test]
    fn merge_materializes_as_a_separate_instruction() {
        let workflow = WorkFlow {
            version: 0,
            nodes: vec![
                normal(0, 1.25, 1.0),
                normal(1, 1.0, 0.75),
                Node {
                    node_id: 2,
                    attr: NodeAttr::Merge {
                        blend_mode: BlendMode::Top,
                    },
                    pos: (0.0, 0.0),
                },
                normal(3, 0.5, 1.0),
            ],
            edges: vec![
                Edge { from: 0, to: 2 },
                Edge { from: 1, to: 2 },
                Edge { from: 2, to: 3 },
            ],
            input_to: vec![0, 1],
            output_from: 3,
        };

        let list = Compiler::new().compile(&workflow).unwrap();
        assert_eq!(
            list.ops
                .iter()
                .filter(|op| matches!(op, Op::Merge { .. }))
                .count(),
            1
        );
        assert_eq!(list.ops.len(), 4);
    }
}

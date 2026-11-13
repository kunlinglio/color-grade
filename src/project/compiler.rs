use std::collections::{HashMap, HashSet};
use std::fmt;

use anyhow::Context;

use super::{NodeKind, Source, WorkFlow};
use crate::render::{BlendMode, Op, PointwiseOp, RenderList, Slot};

pub(super) struct Compiler {
    optimize: bool,
}

impl Compiler {
    pub(super) fn new(optimize: bool) -> Self {
        Self { optimize }
    }

    pub(super) fn compile(&self, workflow: &WorkFlow) -> anyhow::Result<RenderList> {
        // Stage 1: Validate workflow
        let Some(useful_nodes) = workflow.prepare_compile() else {
            // not finished, give up compiling
            anyhow::bail!("Workflow not finished");
        };

        // Stage 2: Lower workflow to IR
        let ir = Lowerer::new(workflow, useful_nodes).lower()?.verify()?;

        // Stage 3: Optimize pass
        let ir = if self.optimize {
            let ir = PointwiseFusing::new(ir).optimize()?.verify()?;
            DeadCodeElimination::new(ir).optimize()?.verify()?
        } else {
            ir
        };
        let ir = ir.verify_materializable()?;

        // Stage 4: Schedule instructions
        let sequence = ir.schedule()?;

        // Stage 5: Materialize IR with scheduled sequence.
        //      Do slot allocation ("Reg allocation") here.
        Materializer::new(&ir, sequence).materialize()
    }
}

struct DeadCodeElimination {
    ir: IR,
}

impl DeadCodeElimination {
    fn new(ir: IR) -> Self {
        Self { ir }
    }

    fn optimize(mut self) -> anyhow::Result<IR> {
        let def_use = IRDefUse::build(&self.ir)?;
        let live = def_use.live_instructions(&self.ir)?;
        for (index, instruction) in self.ir.instructions.iter_mut().enumerate() {
            if instruction.is_some() && !live.contains(&InstructionId(index)) {
                *instruction = None;
            }
        }
        Ok(self.ir)
    }
}

struct PointwiseFusing {
    ir: IR,
}

impl PointwiseFusing {
    pub fn new(ir: IR) -> Self {
        Self { ir }
    }
    pub fn optimize(mut self) -> anyhow::Result<IR> {
        let mut def_use = IRDefUse::build(&self.ir)?;
        for instruction_id in self.ir.schedule()? {
            let pre_instruction_id = {
                let Some(instruction) = self.ir.instructions[instruction_id.0].as_ref() else {
                    continue;
                };
                let IROperation::Pointwise(_) = &instruction.operation else {
                    // cannot fuse non-pointwise instructions
                    continue;
                };
                assert!(
                    instruction.inputs.len() == 1,
                    "pointwise instruction is not unary"
                );
                let input = instruction.inputs[0];
                let Some(pre_instruction_id) = def_use.values[input.0].producer else {
                    // instruction's input comes from "input"
                    continue;
                };
                let Some(pre_instruction) = self.ir.instructions[pre_instruction_id.0].as_ref()
                else {
                    continue;
                };
                let IROperation::Pointwise(_) = &pre_instruction.operation else {
                    // previous instruction is not pointwise
                    continue;
                };
                if input == self.ir.output {
                    // The producer defines the observable return value and cannot be removed.
                    // This indicates that current instruction is just a dead code.
                    continue;
                }
                if def_use.values[input.0].consumer.len() != 1 {
                    // Other consumers still need the producer's output.
                    continue;
                }
                pre_instruction_id
            };

            // Move both instructions out; the producer's slot becomes `None`.
            let current = self.ir.instructions[instruction_id.0]
                .take()
                .expect("scheduled instruction must be active");
            let previous = self.ir.instructions[pre_instruction_id.0]
                .take()
                .expect("producer must be active");

            let (operations, inputs) = match (previous.operation, current.operation) {
                (IROperation::Pointwise(mut previous_ops), IROperation::Pointwise(current_ops)) => {
                    previous_ops.extend(current_ops);
                    (previous_ops, previous.inputs)
                }
                _ => unreachable!("both instructions were checked to be pointwise"),
            };

            self.ir.instructions[instruction_id.0] = Some(IRInstruction {
                operation: IROperation::Pointwise(operations),
                inputs,
                output: current.output,
            });
            // Rebuild def-use because ir has changed
            // TODO: Optimize this rebuild
            def_use = IRDefUse::build(&self.ir)?;
        }
        self.ir = self.ir.verify()?;
        Ok(self.ir)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ValueId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct InstructionId(usize);

#[derive(Debug, Clone)]
struct IRInstruction {
    operation: IROperation,
    inputs: Vec<ValueId>,
    output: ValueId,
}

#[derive(Debug, Clone)]
enum IROperation {
    Pointwise(Vec<PointwiseOp>),
    Merge(BlendMode),
}

#[derive(Clone)]
struct IRDefUseEntry {
    producer: Option<InstructionId>,
    consumer: Vec<InstructionId>,
}

/// Def use analysis for IR
struct IRDefUse {
    values: Vec<IRDefUseEntry>,
}

impl IRDefUse {
    fn build(ir: &IR) -> anyhow::Result<Self> {
        let mut values = vec![
            IRDefUseEntry {
                producer: None,
                consumer: Vec::new()
            };
            ir.max_value_id
        ];
        // First collect every definition. Uses are validated in a second pass so
        // the IR does not have to store producers before their consumers.
        for (index, instruction) in ir.instructions.iter().enumerate() {
            let Some(instruction) = instruction else {
                continue;
            };
            let id = InstructionId(index);
            let output = instruction.output.0;
            anyhow::ensure!(
                output < ir.max_value_id,
                "Defined value %{} out of bound {}",
                output,
                ir.max_value_id
            );
            anyhow::ensure!(
                values[output].producer.is_none() && output != ir.input.0,
                "Assign to value %{} multiple times",
                output
            );
            values[output].producer = Some(id);
        }

        for (index, instruction) in ir.instructions.iter().enumerate() {
            let Some(instruction) = instruction else {
                continue;
            };
            let id = InstructionId(index);
            for &input in &instruction.inputs {
                anyhow::ensure!(
                    input.0 < ir.max_value_id,
                    "Use value %{} out of bound {}",
                    input.0,
                    ir.max_value_id
                );
                anyhow::ensure!(
                    input == ir.input || values[input.0].producer.is_some(),
                    "Use undefined value %{}",
                    input.0
                );
                if values[input.0].consumer.contains(&id) {
                    continue;
                };
                values[input.0].consumer.push(id);
            }
        }
        Ok(Self { values })
    }

    fn live_instructions(&self, ir: &IR) -> anyhow::Result<HashSet<InstructionId>> {
        let mut live = HashSet::new();
        let mut values = vec![ir.output];
        while let Some(value) = values.pop() {
            if value == ir.input {
                continue;
            }
            let producer = self.values[value.0]
                .producer
                .with_context(|| format!("Value %{} has no producer", value.0))?;
            if !live.insert(producer) {
                continue;
            }
            let instruction = ir.instructions[producer.0]
                .as_ref()
                .context("Value producer is not active")?;
            values.extend(instruction.inputs.iter().copied());
        }
        Ok(live)
    }
}

#[derive(Debug)]
struct IR {
    /// `None` marks an instruction removed by an optimization pass. The slot is
    /// kept so that instruction ids (which are indices) stay stable.
    instructions: Vec<Option<IRInstruction>>,
    input: ValueId,
    output: ValueId,
    /// Point to next available value id
    max_value_id: usize,
}

impl IR {
    fn new() -> Self {
        let input = ValueId(0);
        Self {
            instructions: Vec::new(),
            input,
            output: input,
            max_value_id: 1usize, // %0 is occupied by input and output
        }
    }

    fn push(&mut self, operation: IROperation, inputs: Vec<ValueId>) -> ValueId {
        let output = ValueId(self.max_value_id);
        self.max_value_id += 1;
        self.instructions.push(Some(IRInstruction {
            operation,
            inputs,
            output,
        }));
        output
    }

    fn verify(self) -> anyhow::Result<IR> {
        // Verify input and output
        anyhow::ensure!(self.input.0 < self.max_value_id, "IR input is out of range");
        anyhow::ensure!(
            self.output.0 < self.max_value_id,
            "IR output is out of range"
        );
        anyhow::ensure!(
            self.instructions
                .iter()
                .flatten()
                .any(|i| i.output == self.output)
                || self.output == self.input,
            "IR output %{} is not produced by any instruction or input",
            self.output.0
        );
        // Verify values
        IRDefUse::build(&self).with_context(|| format!("Failed to verify IR: {}", self))?;
        // Verify operand count
        for (index, instruction) in self.instructions.iter().enumerate() {
            let Some(instruction) = instruction else {
                continue;
            };
            match &instruction.operation {
                IROperation::Pointwise(ops) => {
                    anyhow::ensure!(
                        instruction.inputs.len() == 1,
                        "pointwise #{index} is not unary"
                    );
                    anyhow::ensure!(!ops.is_empty(), "pointwise #{index} has no operations");
                }
                IROperation::Merge(_) => {
                    anyhow::ensure!(
                        instruction.inputs.len() == 2,
                        "merge #{index} is not binary"
                    );
                }
            }
        }
        // Verify the active instruction graph is acyclic.
        self.schedule()?;
        Ok(self)
    }

    /// Verify if all code is useful.
    /// Containing dead code is allowed as intermediate IR, but not allowed before materialized.
    fn verify_materializable(self) -> anyhow::Result<IR> {
        let def_use = IRDefUse::build(&self)?;
        let live = def_use.live_instructions(&self)?;
        let dead: Vec<usize> = self
            .instructions
            .iter()
            .enumerate()
            .filter_map(|(index, instruction)| {
                (instruction.is_some() && !live.contains(&InstructionId(index))).then_some(index)
            })
            .collect();
        anyhow::ensure!(dead.is_empty(), "IR contains dead instructions: {dead:?}");
        Ok(self)
    }

    /// Use kahn LIFO to schedule instructions
    /// TODO: use a better scheduler algorithm
    fn schedule(&self) -> anyhow::Result<Vec<InstructionId>> {
        let def_use = IRDefUse::build(self)?;

        let mut indegree = vec![0usize; self.instructions.len()];
        let mut active_count = 0;
        for (index, instruction) in self.instructions.iter().enumerate() {
            let Some(instruction) = instruction else {
                continue;
            };
            active_count += 1;
            let mut producers = HashSet::new();
            for input in &instruction.inputs {
                if let Some(producer) = def_use.values[input.0].producer {
                    producers.insert(producer);
                }
            }
            indegree[index] = producers.len();
        }

        let mut ready: Vec<InstructionId> = self
            .instructions
            .iter()
            .enumerate()
            .filter(|(index, instruction)| instruction.is_some() && indegree[*index] == 0)
            .map(|(index, _)| InstructionId(index))
            .collect();
        let mut res = Vec::with_capacity(active_count);

        while let Some(instruction_id) = ready.pop() {
            res.push(instruction_id);
            let instruction = self.instructions[instruction_id.0]
                .as_ref()
                .expect("scheduled instruction must be active");
            let output = instruction.output;
            for &consumer in &def_use.values[output.0].consumer {
                indegree[consumer.0] -= 1;
                if indegree[consumer.0] == 0 {
                    ready.push(consumer);
                }
            }
        }

        anyhow::ensure!(res.len() == active_count, "instruction IR contains a cycle");
        Ok(res)
    }
}

impl fmt::Display for IR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ir(%{}) {{", self.input.0)?;

        for instruction in self.instructions.iter().flatten() {
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

struct Lowerer<'workflow> {
    workflow: &'workflow WorkFlow,
    ir: IR,
    outputs: HashMap<usize, ValueId>,
    useful_nodes: HashSet<usize>,
}

impl<'workflow> Lowerer<'workflow> {
    fn new(workflow: &'workflow WorkFlow, useful_nodes: HashSet<usize>) -> Self {
        Self {
            workflow,
            ir: IR::new(),
            outputs: HashMap::new(),
            useful_nodes,
        }
    }

    fn lower(mut self) -> anyhow::Result<IR> {
        for &node_index in self.workflow.graph().topological_order() {
            if self.useful_nodes.contains(&node_index) {
                self.lower_node(node_index)?;
            }
        }
        self.ir.output = self.value_of(self.workflow.output());
        Ok(self.ir)
    }

    fn lower_node(&mut self, node_index: usize) -> anyhow::Result<()> {
        let node = &self.workflow.nodes()[node_index];
        let output = match &node.attr {
            NodeKind::Normal {
                source,
                brightness,
                saturation,
            } => {
                let mut current = self.value_of(source.expect("Normal source must be reachable"));
                if *brightness != 1.0 {
                    current = self.ir.push(
                        IROperation::Pointwise(vec![PointwiseOp::Brightness(*brightness)]),
                        vec![current],
                    );
                }
                if *saturation != 1.0 {
                    current = self.ir.push(
                        IROperation::Pointwise(vec![PointwiseOp::Saturation(*saturation)]),
                        vec![current],
                    );
                }
                current
            }
            NodeKind::Merge {
                sources,
                blend_mode,
            } => {
                let mut sources = sources.iter().copied();
                let mut current = self.value_of(sources.next().expect("Merge must have inputs"));
                for source in sources {
                    let foreground = self.value_of(source);
                    current = self
                        .ir
                        .push(IROperation::Merge(*blend_mode), vec![foreground, current]);
                }
                current
            }
            NodeKind::Generator { .. } => {
                todo!("Implement generator's lower")
            }
        };

        self.outputs.insert(node_index, output);
        Ok(())
    }

    fn value_of(&self, source: Source) -> ValueId {
        match source {
            Source::Input => self.ir.input,
            Source::Node(id) => {
                let index = self
                    .workflow
                    .graph()
                    .index_of(id)
                    .expect("Source node must exist");
                self.outputs
                    .get(&index)
                    .copied()
                    .expect(&format!("Source node {id} is not lowered"))
            }
        }
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
        let mut use_counts = vec![0usize; ir.max_value_id];
        for &instruction_id in &sequence {
            let instruction = ir.instructions[instruction_id.0]
                .as_ref()
                .expect("scheduled instruction must be active");
            for input in &instruction.inputs {
                use_counts[input.0] += 1;
            }
        }
        // Keep the program output alive for RenderList's epilogue.
        use_counts[ir.output.0] += 1;

        Self {
            ir,
            sequence,
            allocator: SlotAllocator::new(),
            value_slots: vec![None; ir.max_value_id],
            use_counts,
            ops: Vec::new(),
        }
    }

    fn materialize(mut self) -> anyhow::Result<RenderList> {
        let input_slot = self.allocator.alloc();
        self.bind_value(self.ir.input, input_slot)?;

        for instruction_id in self.sequence.clone() {
            let instruction = self.ir.instructions[instruction_id.0]
                .as_ref()
                .expect("scheduled instruction must be active");
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
                    assert!(input_slots.len() == 1, "pointwise op is not unary");
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
                    assert!(input_slots.len() == 2, "merge op is not binary");
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

    /// Bind a slot to a value and increase the slot's ref count for all users
    fn bind_value(&mut self, value: ValueId, slot: Slot) -> anyhow::Result<()> {
        let uses = self.use_counts[value.0];
        assert!(uses > 0, "materialized value {} has no uses", value.0);
        assert!(self.value_slots[value.0].is_none(), "value bound twice");
        for _ in 1..uses {
            self.allocator.dup(slot);
        }
        self.value_slots[value.0] = Some(slot);
        Ok(())
    }
}

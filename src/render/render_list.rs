use std::fmt;

use anyhow::Context;
use strum::EnumIter;

/// Render List is an important IR (Intermediate Representation) in render pipeline.
/// It contains:
/// - a sequence of Ops: instructions
/// - max slot count: max "registers" backend can prealloc in ahead
/// - input & output: calling convention, specified which slot can be used
///   for input and output
///   PS: The register move in ISA is pretty cheap but expensive in our case, so we
///       use "Configuration" over "Convention". :)
///
/// Since all backend share one Render List definition, it should not have any
/// assumption to a particular consumer.
///
/// NOTE: Every frames in Intermediate (not all Slots) should follow:
///      - AP1 Color Gamut with D60
///      - ACEScct
///      - RGBA: [f32; 4] precision with straight alpha
#[derive(Debug)]
pub struct RenderList {
    /// The frame buffers slots backend need to pre alloc.
    /// These slots will used as operands in the ops.
    pub slot_count: usize,
    pub ops: Vec<Op>,
    /// The target to copy input to.
    pub input: Slot,
    /// The source to copy output from.
    pub output: Slot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(u32);

impl Slot {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
    pub fn to_index(&self) -> usize {
        self.0 as usize
    }
    fn inside(&self, slot_count: usize) -> bool {
        (self.0 as usize) < slot_count
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, Default)]
pub enum BlendMode {
    /// Put fg above bg
    #[default]
    Normal,
    Add,
    Subtract,
    Multiply,
    Screen,
    Overlay,
    Difference,
    Darken,
    Lighten,
}

impl fmt::Display for BlendMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            BlendMode::Normal => "normal",
            BlendMode::Add => "add",
            BlendMode::Subtract => "subtract",
            BlendMode::Multiply => "multiply",
            BlendMode::Screen => "screen",
            BlendMode::Overlay => "overlay",
            BlendMode::Difference => "difference",
            BlendMode::Darken => "darken",
            BlendMode::Lighten => "lighten",
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointwiseOp {
    Brightness(f32),
    Saturation(f32),
}

#[derive(Debug)]
pub enum Op {
    /// TODO: Make input and output convert a legal pointwise op.
    ///       e.g. u8 -> f32 ... f32 -> u8
    Pointwise {
        from: Slot,
        to: Slot,
        operations: Vec<PointwiseOp>,
    },
    Merge {
        fg: Slot,
        bg: Slot,
        to: Slot,
        mode: BlendMode,
    },
    Mix {
        base: Slot,
        from: Vec<Slot>,
        to: Slot,
    },
}

impl RenderList {
    /// Verify that every op only uses in-range slots and is well formed.
    pub fn verify(&self) -> anyhow::Result<()> {
        self.check_slot(&self.input, "input")?;
        self.check_slot(&self.output, "output")?;
        for (index, op) in self.ops.iter().enumerate() {
            self.verify_op(op)
                .with_context(|| format!("invalid op #{index}: {op:?}"))?;
        }
        Ok(())
    }

    fn verify_op(&self, op: &Op) -> anyhow::Result<()> {
        match op {
            Op::Pointwise {
                from,
                to,
                operations,
            } => {
                self.check_slot(from, "from")?;
                self.check_slot(to, "to")?;
                anyhow::ensure!(
                    !operations.is_empty(),
                    "Pointwise must contain at least one operation"
                );
            }
            Op::Merge { fg, bg, to, .. } => {
                self.check_slot(fg, "fg")?;
                self.check_slot(bg, "bg")?;
                self.check_slot(to, "to")?;
            }
            Op::Mix { base, from, to } => {
                self.check_slot(base, "base")?;
                anyhow::ensure!(from.len() >= 2, "Mix requires at least two branches");
                for (index, slot) in from.iter().enumerate() {
                    self.check_slot(slot, &format!("from: %{index}"))?;
                }
                self.check_slot(to, "to")?;
            }
        }
        Ok(())
    }

    fn check_slot(&self, slot: &Slot, name: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            slot.inside(self.slot_count),
            "{name} slot {} is out of range (slot_count = {})",
            slot.id(),
            self.slot_count
        );
        Ok(())
    }
}

impl fmt::Display for RenderList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "render_list({}, slots = {}) {{",
            self.input, self.slot_count
        )?;
        for op in &self.ops {
            match op {
                Op::Pointwise {
                    from,
                    to,
                    operations,
                } => {
                    write!(f, "  {to} = pointwise [")?;
                    for (index, operation) in operations.iter().enumerate() {
                        if index != 0 {
                            write!(f, ", ")?;
                        }
                        match operation {
                            PointwiseOp::Brightness(value) => write!(f, "brightness({value:?})")?,
                            PointwiseOp::Saturation(value) => write!(f, "saturation({value:?})")?,
                        }
                    }
                    writeln!(f, "] {from}")?;
                }
                Op::Merge { fg, bg, to, mode } => {
                    writeln!(f, "  {to} = merge.{mode} {bg}, {fg}")?;
                }
                Op::Mix { base, from, to } => {
                    write!(f, "  {to} = mix base {base}, branches [")?;
                    for (index, branch) in from.iter().enumerate() {
                        if index != 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{branch}")?;
                    }
                    writeln!(f, "]")?;
                }
            }
        }
        writeln!(f, "  ret {}", self.output)?;
        write!(f, "}}")
    }
}

use anyhow::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(u32);

impl Slot {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    pub fn id(&self) -> u32 {
        self.0
    }
    fn inside(&self, slot_count: usize) -> bool {
        (self.0 as usize) < slot_count
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Top,
    // TODO: Add more blend mode
}

#[derive(Debug)]
pub enum Op {
    Copy {
        from: Slot,
        to: Slot,
    },
    Brightness {
        from: Slot,
        to: Slot,
        brightness: f32,
    },
    Saturation {
        from: Slot,
        to: Slot,
        saturation: f32,
    },
    Merge {
        fg: Slot,
        bg: Slot,
        to: Slot,
        mode: BlendMode,
    },
}

impl RenderList {
    /// Verify that every op only uses in-range slots and is well formed.
    pub fn verify(&self) -> anyhow::Result<()> {
        self.check_slot(&self.input, "input");
        self.check_slot(&self.output, "output");
        for (index, op) in self.ops.iter().enumerate() {
            self.verify_op(op)
                .with_context(|| format!("invalid op #{index}: {op:?}"))?;
        }
        Ok(())
    }

    fn verify_op(&self, op: &Op) -> anyhow::Result<()> {
        match op {
            Op::Copy { from, to } => {
                self.check_slot(from, "from")?;
                self.check_slot(to, "to")?;
                anyhow::ensure!(from != to, "from and to must be different slots");
            }
            Op::Brightness { from, to, .. } | Op::Saturation { from, to, .. } => {
                self.check_slot(from, "from")?;
                self.check_slot(to, "to")?;
                anyhow::ensure!(from != to, "from and to must be different slots");
            }
            Op::Merge { fg, bg, to, .. } => {
                self.check_slot(fg, "fg")?;
                self.check_slot(bg, "bg")?;
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

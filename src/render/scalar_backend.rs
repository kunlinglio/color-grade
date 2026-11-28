use std::sync::Arc;

use anyhow::{Context, bail};

use super::cpu_color::CPUColorPipeline;
use super::cpu_kernel::{self, CPUFrame};
use super::render_backend::RenderBackend;
use super::render_backend::ScopeGraph;
use super::render_list::{Op, RenderList, Slot};
use super::{BackendFrame, DecodedImage, RenderedFrame};

/// The most basic render backend. It deliberately favors prioritize over
/// performance and serves as the reference implementation for regression tests
/// against optimized backends.
pub struct ScalarBackend {
    color_pipe: CPUColorPipeline,
}

impl ScalarBackend {
    pub fn new() -> Self {
        let color_pipe = CPUColorPipeline::new();
        Self { color_pipe }
    }

    fn execute(
        &self,
        list: RenderList,
        from: &DecodedImage,
    ) -> anyhow::Result<cpu_kernel::CPUFrame> {
        list.verify().context("invalid render list")?;
        let RenderList {
            slot_count,
            ops,
            input,
            output,
        } = list;

        // Slots are stored as `Option<ScalarFrame>` so that reading a slot that was
        // never written is reported instead of silently producing garbage.
        let mut slots: Vec<Option<cpu_kernel::CPUFrame>> = vec![None; slot_count];

        // Prepare input
        let input_index = input.id() as usize;
        let seed = slots.get_mut(input_index).with_context(|| {
            format!("input slot {input_index} out of range (slot_count = {slot_count})")
        })?;
        let mut input = cpu_kernel::CPUFrame::from_frame(from);
        self.color_pipe.apply_input(&mut input.rgba, &from.icc);
        *seed = Some(input);

        // Execute body
        for (op_index, op) in ops.iter().enumerate() {
            match op {
                Op::Pointwise {
                    from,
                    to,
                    operations,
                } => {
                    if from == to {
                        cpu_kernel::apply_pointwise(
                            &mut slot_mut(&mut slots, *to, op_index, "to")?.rgba,
                            operations,
                        );
                    } else {
                        let mut value = read_slot(&slots, *from, op_index, "from")?.clone();
                        cpu_kernel::apply_pointwise(&mut value.rgba, operations);
                        write_slot(&mut slots, *to, value, op_index)?;
                    }
                }
                Op::Merge { fg, bg, to, mode } => {
                    let foreground = read_slot(&slots, *fg, op_index, "fg")?;
                    let background = read_slot(&slots, *bg, op_index, "bg")?;
                    let mut to_frame = CPUFrame::new(foreground.width, foreground.height);
                    cpu_kernel::merge(
                        &foreground.rgba,
                        &background.rgba,
                        *mode,
                        &mut to_frame.rgba,
                    )?;
                    write_slot(&mut slots, *to, to_frame, op_index)?;
                }
                Op::Mix { base, from, to } => {
                    let base = read_slot(&slots, *base, op_index, "base")?;
                    let branches = from
                        .iter()
                        .map(|slot| {
                            read_slot(&slots, *slot, op_index, "from")
                                .and_then(|frame| Ok(frame.rgba.as_slice()))
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let mut to_frame = CPUFrame::new(base.width, base.height);
                    cpu_kernel::mix(&base.rgba, &branches, &mut to_frame.rgba)?;

                    write_slot(&mut slots, *to, to_frame, op_index)?;
                }
            }
        }

        // Take output
        let output_index = output.id() as usize;
        let len = slots.len();
        slots
            .get_mut(output_index)
            .with_context(|| {
                format!("output slot {output_index} out of range (slot_count = {len})")
            })?
            .take()
            .with_context(|| format!("output slot {output_index} was never written"))
            .map(|mut frame| {
                self.color_pipe.apply_output(&mut frame.rgba);
                frame
            })
    }
}

/// Read a slot, failing if it is out of range or was never written.
fn read_slot<'a>(
    slots: &'a [Option<cpu_kernel::CPUFrame>],
    slot: Slot,
    op_index: usize,
    name: &str,
) -> anyhow::Result<&'a cpu_kernel::CPUFrame> {
    let index = slot.id() as usize;
    match slots.get(index) {
        Some(Some(frame)) => Ok(frame),
        Some(None) => bail!("op #{op_index} reads uninitialized slot {index} ({name})"),
        None => bail!(
            "op #{op_index} reads {name} slot {index} out of range (slot_count = {})",
            slots.len()
        ),
    }
}

/// Get mutable access to an initialized slot, for in-place ops.
fn slot_mut<'a>(
    slots: &'a mut [Option<cpu_kernel::CPUFrame>],
    slot: Slot,
    op_index: usize,
    name: &str,
) -> anyhow::Result<&'a mut cpu_kernel::CPUFrame> {
    let index = slot.id() as usize;
    let len = slots.len();
    let cell = slots.get_mut(index).with_context(|| {
        format!("op #{op_index} writes {name} slot {index} out of range (slot_count = {len})")
    })?;
    match cell {
        Some(frame) => Ok(frame),
        None => bail!("op #{op_index} modifies uninitialized slot {index} ({name})"),
    }
}

/// Write a slot, failing if it is out of range.
fn write_slot(
    slots: &mut [Option<cpu_kernel::CPUFrame>],
    slot: Slot,
    frame: cpu_kernel::CPUFrame,
    op_index: usize,
) -> anyhow::Result<()> {
    let index = slot.id() as usize;
    let len = slots.len();
    let dst = slots.get_mut(index).with_context(|| {
        format!("op #{op_index} writes slot {index} out of range (slot_count = {len})")
    })?;
    *dst = Some(frame);
    Ok(())
}

impl RenderBackend for ScalarBackend {
    fn handle_render(
        &self,
        list: RenderList,
        from: Arc<DecodedImage>,
    ) -> (BackendFrame, Arc<RenderedFrame>) {
        let buffer = self
            .execute(list, &from)
            .unwrap_or_else(|err| panic!("scalar backend failed to render: {err:#}"));
        let mut rendered = RenderedFrame::new(buffer.width, buffer.height);
        cpu_kernel::render_frame(&buffer.rgba, true, &mut rendered.bgra, 0);
        (BackendFrame::new(buffer), Arc::new(rendered))
    }

    fn handle_scope(&self, from: BackendFrame, width: usize, height: usize) -> ScopeGraph {
        let frame = from.downcast::<cpu_kernel::CPUFrame>();
        let waveform = Arc::new(cpu_kernel::waveform(frame.as_ref(), width, height));
        let vectorscope = Arc::new(cpu_kernel::vectorscope(frame.as_ref(), width, height));
        let (y_histogram, rgb_histogram) = cpu_kernel::histogram(frame.as_ref(), width, height);
        ScopeGraph {
            waveform,
            vectorscope,
            y_histogram: Arc::new(y_histogram),
            rgb_histogram: Arc::new(rgb_histogram),
        }
    }

    fn render_thumbnail(&self, from: Arc<DecodedImage>) -> Arc<RenderedFrame> {
        let mut buffer = cpu_kernel::CPUFrame::from_frame(&from);
        self.color_pipe.apply_input(&mut buffer.rgba, &from.icc);
        self.color_pipe.apply_output(&mut buffer.rgba);
        let mut rendered = RenderedFrame::new(buffer.width, buffer.height);
        cpu_kernel::render_frame(&buffer.rgba, true, &mut rendered.bgra, 0);
        Arc::new(rendered)
    }
}

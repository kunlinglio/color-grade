use std::sync::Arc;

use anyhow::Context;
use rayon::ThreadPool;
use rayon::iter::{IndexedParallelIterator, ParallelIterator};
use rayon::prelude::*;

use super::cpu_color::CPUColorPipeline;
use super::cpu_kernel::{self, CPUFrame};
use super::{BackendFrame, DecodedImage, MediaIcc, Op, RenderBackend, RenderList, RenderedFrame};

/// The high-performance CPU backend implementation.
/// Different with ScalarBackend, this implementation introduced many optimization
/// to improve its performance on CPU.
pub struct CPUBackend {
    thread_pool: Arc<ThreadPool>,
    color_pipe: CPUColorPipeline,
}

impl CPUBackend {
    pub(super) fn new(thread_pool: Arc<ThreadPool>) -> Self {
        let color_pipe = CPUColorPipeline::new();
        Self {
            thread_pool,
            color_pipe,
        }
    }

    fn rows_per_tile(&self, width: usize, height: usize) -> usize {
        let threads = self.thread_pool.current_num_threads();
        if threads == 1 {
            return height;
        }
        if width * height < 4096 {
            return height;
        };
        height.div_ceil(threads * 4).max(1)
    }

    fn execute_tile(
        &self,
        list: &RenderList,
        from: &[u16],
        from_icc: &Option<MediaIcc>,
        cpu_frame: &mut [f32],
        rendered: &mut [u8],
        pixel_offset: usize,
    ) -> anyhow::Result<()> {
        let tile_len = from.len();
        let RenderList {
            slot_count,
            ops,
            input,
            output,
        } = &*list;
        // TODO: Use sequent arena.
        let mut slots: Vec<Vec<f32>> = vec![vec![0.0; tile_len]; *slot_count];

        // Prepare input
        let input_index = input.id() as usize;
        let seed = slots.get_mut(input_index).with_context(|| {
            format!("input slot {input_index} out of range (slot_count = {slot_count})")
        })?;
        for (i, e) in from.iter().enumerate() {
            seed[i] = *e as f32 / u16::MAX as f32;
        }
        self.color_pipe.apply_input(seed, &from_icc);

        // Execute body
        for op in ops {
            match op {
                Op::Pointwise {
                    from,
                    to,
                    operations,
                } => {
                    if from == to {
                        cpu_kernel::apply_pointwise(&mut slots[from.to_index()], operations);
                    } else {
                        let (target, remains) = get_mut_slot(&mut slots, to.to_index());
                        let source = get_slot(&remains, to.to_index(), from.to_index());
                        target.copy_from_slice(source);
                        cpu_kernel::apply_pointwise(target, operations);
                    }
                }
                Op::Merge { fg, bg, to, mode } => {
                    assert!(to != fg && to != bg);
                    let (target, remains) = get_mut_slot(&mut slots, to.to_index());
                    let foreground = get_slot(&remains, to.to_index(), fg.to_index());
                    let background = get_slot(&remains, to.to_index(), bg.to_index());
                    cpu_kernel::merge(foreground, background, *mode, target)?;
                }
                Op::Mix { base, from, to } => {
                    assert!(to != base && !from.contains(to));
                    let (target, remains) = get_mut_slot(&mut slots, to.to_index());
                    let base = get_slot(&remains, to.to_index(), base.to_index());
                    let branches: Vec<&[f32]> = from
                        .iter()
                        .map(|slot| get_slot(&remains, to.to_index(), slot.to_index()))
                        .collect();
                    cpu_kernel::mix(base, &branches, target)?;
                }
            }
        }

        // Take output
        let output = &mut slots[output.id() as usize];
        self.color_pipe.apply_output(output);
        cpu_frame.copy_from_slice(output);
        cpu_kernel::render_frame(output, true, rendered, pixel_offset);
        Ok(())
    }
}

struct RemainsSlot<'a> {
    before: &'a [Vec<f32>],
    after: &'a [Vec<f32>],
}

fn get_mut_slot<'a>(
    slots: &'a mut [Vec<f32>],
    index: usize,
) -> (&'a mut Vec<f32>, RemainsSlot<'a>) {
    let (left, target_and_right) = slots.split_at_mut(index);
    let (target, right) = target_and_right
        .split_first_mut()
        .expect("output slot out of range");
    (
        target,
        RemainsSlot {
            before: left,
            after: right,
        },
    )
}

fn get_slot<'a>(remains: &'a RemainsSlot, mut_index: usize, index: usize) -> &'a [f32] {
    if index < mut_index {
        &remains.before[index][..]
    } else {
        &remains.after[index - mut_index - 1][..]
    }
}

impl RenderBackend for CPUBackend {
    fn handle_render(
        &self,
        list: RenderList,
        from: Arc<DecodedImage>,
    ) -> (BackendFrame, Arc<RenderedFrame>) {
        list.verify().expect("invalid render list");

        // Tiled picture
        let (width, height) = from.size();
        let tile_rows = self.rows_per_tile(width, height);
        let tile_pixels = tile_rows * width;
        let tile_len = tile_pixels * 4;
        let mut cpu_frame = CPUFrame::new(width, height);
        let mut rendered_frame = RenderedFrame::new(width, height);
        self.thread_pool.install(|| {
            from.rgba
                .par_chunks(tile_len)
                .zip(cpu_frame.rgba.par_chunks_mut(tile_len))
                .zip(rendered_frame.bgra.par_chunks_mut(tile_len))
                .enumerate()
                .for_each({
                    let from_icc = &from.icc;
                    move |(i, ((from, cpu_frame), rendered))| {
                        let pixel_offset = i * tile_pixels;
                        self.execute_tile(&list, from, from_icc, cpu_frame, rendered, pixel_offset)
                            .with_context(|| {
                                format!("Failed to render render list on CPUBackend: {list}")
                            })
                            .unwrap_or_else(|e| panic!("CPU tile failed: {e:#}"))
                    }
                });
        });
        (BackendFrame::new(cpu_frame), Arc::new(rendered_frame))
    }
    fn handle_scope(&self, from: BackendFrame, width: usize, height: usize) -> super::ScopeGraph {
        todo!()
    }
    fn render_thumbnail(&self, from: Arc<DecodedImage>) -> Arc<RenderedFrame> {
        todo!()
    }
}

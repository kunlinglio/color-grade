use std::sync::Arc;

use super::render_list::{Op, RenderList};
use super::renderer::RendererBackend;
use super::renderer::ScopeGraph;
use super::{BackendFrame, Frame, RenderedFrame};

/// The most basic render backend. It deliberately favors correctness over
/// performance and serves as the reference implementation for regression tests
/// against optimized backends.
pub(super) struct ScalarBackend {}

impl ScalarBackend {
    pub fn new() -> Self {
        Self {}
    }
}

impl RendererBackend for ScalarBackend {
    fn handle_render(
        &self,
        list: RenderList,
        from: Arc<Frame>,
    ) -> (BackendFrame, Arc<RenderedFrame>) {
        let mut buffer = ScalarFrame::from_frame(&from);
        for op in list.ops {
            match op {
                Op::Brightness(b) => {
                    kernel::change_brightness(&mut buffer, b);
                }
                Op::Saturation(s) => {
                    kernel::change_saturation(&mut buffer, s);
                }
            }
        }
        let rendered = RenderedFrame::from_frame(buffer.to_frame());
        (BackendFrame::new(buffer), Arc::new(rendered))
    }

    fn handle_scope(&self, from: BackendFrame, width: usize, height: usize) -> ScopeGraph {
        let frame = from.downcast::<ScalarFrame>();
        let waveform = Arc::new(kernel::waveform(frame.as_ref(), width, height));
        let vectorscope = Arc::new(kernel::vectorscope(frame.as_ref(), width, height));
        let (y_histogram, rgb_histogram) = kernel::histogram(frame.as_ref(), width, height);
        ScopeGraph {
            waveform,
            vectorscope,
            y_histogram: Arc::new(y_histogram),
            rgb_histogram: Arc::new(rgb_histogram),
        }
    }
}

/// Intermediate frame representation used between processing steps.
struct ScalarFrame {
    width: usize,
    height: usize,
    rgba: Vec<f32>,
}

impl ScalarFrame {
    fn new(width: usize, height: usize) -> Self {
        let mut rgba = vec![0.0f32; width * height * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255.0; // let alpha = 255
        }
        ScalarFrame {
            width,
            height,
            rgba: rgba,
        }
    }
    fn from_frame(frame: &Frame) -> Self {
        ScalarFrame {
            rgba: frame.rgba.iter().map(|&e| e as f32).collect(),
            width: frame.width,
            height: frame.height,
        }
    }

    fn to_frame(&self) -> Frame {
        Frame {
            rgba: self.rgba.iter().map(|&e| e as u8).collect(),
            width: self.width,
            height: self.height,
        }
    }
}

mod kernel {
    use super::ScalarFrame;
    use crate::render::RenderedFrame;

    const MAX: f32 = 255.0;

    fn luma(r: f32, g: f32, b: f32) -> f32 {
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    pub(super) fn change_brightness(buffer: &mut ScalarFrame, b: f32) {
        let factor = b.max(0.0);
        for px in buffer.rgba.chunks_exact_mut(4) {
            px[0] *= factor;
            px[1] *= factor;
            px[2] *= factor;
        }
    }

    pub(super) fn change_saturation(buffer: &mut ScalarFrame, s: f32) {
        for px in buffer.rgba.chunks_exact_mut(4) {
            let y = luma(px[0], px[1], px[2]);
            px[0] = y + s * (px[0] - y);
            px[1] = y + s * (px[1] - y);
            px[2] = y + s * (px[2] - y);
        }
    }

    pub(super) fn histogram(
        from: &ScalarFrame,
        width: usize,
        height: usize,
    ) -> (RenderedFrame, RenderedFrame) {
        assert!(width > 0 && height > 0);
        // Analyze
        let mut r_hist = vec![0 as usize; width];
        let mut g_hist = vec![0 as usize; width];
        let mut b_hist = vec![0 as usize; width];
        let mut y_hist = vec![0 as usize; width];

        for px in from.rgba.chunks_exact(4) {
            let to_slot = |x: f32| {
                (x / MAX * (width - 1) as f32)
                    .round()
                    .clamp(0.0, (width - 1) as f32) as usize
            };
            let y = to_slot(luma(px[0], px[1], px[2]));
            let r = to_slot(px[0]);
            let g = to_slot(px[1]);
            let b = to_slot(px[2]);

            y_hist[y] += 1;
            r_hist[r] += 1;
            g_hist[g] += 1;
            b_hist[b] += 1;
        }
        // Draw
        // 1. calculate max height
        let rgb_max_height = r_hist
            .iter()
            .chain(g_hist.iter())
            .chain(b_hist.iter())
            .copied()
            .max()
            .unwrap_or(0);
        let y_max_height = y_hist.iter().copied().max().unwrap_or(0);
        let mut rgb_buffer = ScalarFrame::new(width, height);
        let mut y_buffer = ScalarFrame::new(width, height);

        // 2. calculate bar height for each channel
        if rgb_max_height != 0 {
            r_hist.iter_mut().for_each(|col| {
                *col = (*col as f32 / rgb_max_height as f32 * (height - 1) as f32).round() as usize
            });
            g_hist.iter_mut().for_each(|col| {
                *col = (*col as f32 / rgb_max_height as f32 * (height - 1) as f32).round() as usize
            });
            b_hist.iter_mut().for_each(|col| {
                *col = (*col as f32 / rgb_max_height as f32 * (height - 1) as f32).round() as usize
            });
        }
        if y_max_height != 0 {
            y_hist.iter_mut().for_each(|col| {
                *col = (*col as f32 / y_max_height as f32 * (height - 1) as f32).round() as usize
            });
        }

        // 3. fill pixels
        if rgb_max_height != 0 {
            r_hist.iter().enumerate().for_each(|(x, bar_height)| {
                for row in 0..*bar_height {
                    let y = height - 1 - row;
                    let index = 4 * (y * width + x);
                    rgb_buffer.rgba[index + 0] = 255.0;
                }
            });
            g_hist.iter().enumerate().for_each(|(x, bar_height)| {
                for row in 0..*bar_height {
                    let y = height - 1 - row;
                    let index = 4 * (y * width + x);
                    rgb_buffer.rgba[index + 1] = 255.0;
                }
            });
            b_hist.iter().enumerate().for_each(|(x, bar_height)| {
                for row in 0..*bar_height {
                    let y = height - 1 - row;
                    let index = 4 * (y * width + x);
                    rgb_buffer.rgba[index + 2] = 255.0;
                }
            });
        }
        if y_max_height != 0 {
            y_hist.iter().enumerate().for_each(|(x, bar_height)| {
                for row in 0..*bar_height {
                    let y = height - 1 - row;
                    let index = 4 * (y * width + x);
                    y_buffer.rgba[index + 0] = 255.0;
                    y_buffer.rgba[index + 1] = 255.0;
                    y_buffer.rgba[index + 2] = 255.0;
                }
            });
        }
        (
            RenderedFrame::from_frame(y_buffer.to_frame()),
            RenderedFrame::from_frame(rgb_buffer.to_frame()),
        )
    }

    pub(super) fn waveform(from: &ScalarFrame, width: usize, height: usize) -> RenderedFrame {
        assert!(width > 0 && height > 0);
        // TODO: Support RGB waveform
        // Analyze
        let mut density = vec![0u32; width * height];
        for row in from.rgba.chunks_exact(4 * from.width) {
            for (x, px) in row.chunks_exact(4).enumerate() {
                let y = luma(px[0], px[1], px[2]);
                let scope_x = x * width / from.width;
                let normalized_y = (y / 255.0).clamp(0.0, 1.0);
                let scope_y = ((1.0 - normalized_y) * (height - 1) as f32).round() as usize;
                density[scope_y * width + scope_x] += 1;
            }
        }
        // Draw
        let max_density = density.iter().copied().max().unwrap_or(0);
        let mut buffer = ScalarFrame::new(width, height);
        if max_density == 0 {
            return RenderedFrame::from_frame(buffer.to_frame());
        }
        for (y, row) in density.chunks_exact(width).enumerate() {
            for (x, p) in row.iter().enumerate() {
                let density = (1.0 + *p as f32).ln() / (1.0 + max_density as f32).ln();
                let index = (y * width + x) * 4;
                buffer.rgba[index] = density * MAX;
                buffer.rgba[index + 1] = density * MAX;
                buffer.rgba[index + 2] = density * MAX;
            }
        }

        RenderedFrame::from_frame(buffer.to_frame())
    }

    pub(super) fn vectorscope(from: &ScalarFrame, width: usize, height: usize) -> RenderedFrame {
        assert!(width > 0 && height > 0);
        // Analyze
        let size = width.min(height);
        let radius = (size - 1) as f32 / 2.0;
        let center_x = (width - 1) as f32 / 2.0;
        let center_y = (height - 1) as f32 / 2.0;
        let mut density = vec![0u32; width * height];
        for row in from.rgba.chunks_exact(4 * from.width) {
            for px in row.chunks_exact(4) {
                let r = px[0] / MAX;
                let g = px[1] / MAX;
                let b = px[2] / MAX;
                let y = luma(r, g, b);
                let cb = (b - y) / 1.8556;
                let cr = (r - y) / 1.5748;
                let x = center_x + cb * radius;
                let y = center_y - cr * radius;
                let x = x.round().clamp(0.0, (width - 1) as f32) as usize;
                let y = y.round().clamp(0.0, (height - 1) as f32) as usize;
                density[y * width + x] += 1;
            }
        }
        // Draw
        let mut buffer = ScalarFrame::new(width, height);
        let max_density = density.iter().copied().max().unwrap_or(0);
        if max_density == 0 {
            return RenderedFrame::from_frame(buffer.to_frame());
        }
        for (y, row) in density.chunks_exact(width).enumerate() {
            for (x, p) in row.iter().enumerate() {
                let density = (1.0 + *p as f32).ln() / (1.0 + max_density as f32).ln();
                // TODO: Support color it
                let index = (y * width + x) * 4;
                buffer.rgba[index] = density * MAX;
                buffer.rgba[index + 1] = density * MAX;
                buffer.rgba[index + 2] = density * MAX;
            }
        }

        RenderedFrame::from_frame(buffer.to_frame())
    }
}

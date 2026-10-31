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
        const MAX: f32 = 255.0;
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
        let mut rgb_max_height = 0;
        let mut y_max_height = 0;
        for w in 0..width {
            y_max_height = y_max_height.max(y_hist[w]);
            rgb_max_height = rgb_max_height.max(r_hist[w]);
            rgb_max_height = rgb_max_height.max(g_hist[w]);
            rgb_max_height = rgb_max_height.max(b_hist[w]);
        }
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
        // Analyze

        // Draw
        let mut buffer = ScalarFrame::new(width, height);

        RenderedFrame::from_frame(buffer.to_frame())
    }

    pub(super) fn vectorscope(from: &ScalarFrame, width: usize, height: usize) -> RenderedFrame {
        // Analyze

        // Draw
        let mut buffer = ScalarFrame::new(width, height);

        RenderedFrame::from_frame(buffer.to_frame())
    }
}

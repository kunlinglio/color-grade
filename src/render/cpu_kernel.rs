use xxhash_rust::xxh3::xxh3_64;

use super::DecodedImage;
use super::{BlendMode, PointwiseOp, RenderedFrame};

/// Intermediate frame representation used between processing steps.
#[derive(Clone, Debug)]
pub(super) struct CPUFrame {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) rgba: Vec<f32>,
}

impl CPUFrame {
    pub(super) fn new(width: usize, height: usize) -> Self {
        let rgba = vec![0.0f32; width * height * 4];
        CPUFrame {
            width,
            height,
            rgba,
        }
    }
    pub(super) fn set_alpha(&mut self) {
        for px in self.rgba.chunks_exact_mut(4) {
            px[3] = 1.0; // let alpha = 1
        }
    }
    /// Convert to f32 and normalize
    pub(super) fn from_frame(frame: &DecodedImage) -> Self {
        CPUFrame {
            rgba: frame
                .rgba
                .iter()
                .map(|&e| e as f32 / u16::MAX as f32)
                .collect(),
            width: frame.width,
            height: frame.height,
        }
    }
}

mod acescct {
    const LOG_SLOPE: f32 = 1.0 / 17.52;
    const LOG_OFFSET: f32 = 9.72 / 17.52;
    const LINEAR_BREAK: f32 = 0.007_812_5; // 2^-7
    const LN_2: f32 = 0.693_147_180_559_945_3;

    const TOE_SLOPE: f32 = LOG_SLOPE / (LINEAR_BREAK * LN_2); // (log2(x))' when x = 2^-7
    const TOE_OFFSET: f32 = LOG_BREAK - TOE_SLOPE * LINEAR_BREAK;
    const LOG_BREAK: f32 = LOG_SLOPE * -7.0 + LOG_OFFSET; // ACEScct(2^-7), -7 = log2(2^-7)

    #[inline(always)]
    pub(super) fn to_linear(y: f32) -> f32 {
        if y <= LOG_BREAK {
            (y - TOE_OFFSET) / TOE_SLOPE
        } else {
            ((y - LOG_OFFSET) / LOG_SLOPE).exp2()
        }
    }

    #[inline(always)]
    pub(super) fn from_linear(x: f32) -> f32 {
        if x <= LINEAR_BREAK {
            TOE_SLOPE * x + TOE_OFFSET
        } else {
            LOG_SLOPE * x.log2() + LOG_OFFSET
        }
    }

    const LUMA: [f32; 3] = [0.27222872, 0.67408177, 0.05368952];

    #[inline(always)]
    pub(super) fn luma(r: f32, g: f32, b: f32) -> f32 {
        LUMA[0] * r + LUMA[1] * g + LUMA[2] * b
    }
}

mod display_p3 {
    const LUMA: [f32; 3] = [0.2289745641, 0.6917385218, 0.0792869141];

    #[inline(always)]
    pub(super) fn luma(r: f32, g: f32, b: f32) -> f32 {
        LUMA[0] * r + LUMA[1] * g + LUMA[2] * b
    }

    const CB_SCALE: f32 = 2.0 * (1.0 - LUMA[2]);
    const CR_SCALE: f32 = 2.0 * (1.0 - LUMA[0]);

    #[inline(always)]
    pub(super) fn to_cb_cr(r: f32, g: f32, b: f32) -> (f32, f32) {
        let y = luma(r, g, b);
        let cb = (b - y) / CB_SCALE;
        let cr = (r - y) / CR_SCALE;
        (cb, cr)
    }

    #[inline(always)]
    pub(super) fn from_cb_cr(y: f32, cb: f32, cr: f32) -> (f32, f32, f32) {
        let r = y + cr * CR_SCALE;
        let b = y + cb * CB_SCALE;
        let g = (y - LUMA[0] * r - LUMA[2] * b) / LUMA[1];
        (r, g, b)
    }
}

pub(super) fn apply_pointwise(buffer: &mut [f32], operations: &[PointwiseOp]) {
    for px in buffer.chunks_exact_mut(4) {
        for operation in operations {
            match operation {
                PointwiseOp::Brightness(brightness) => {
                    let factor = brightness.max(0.0);
                    for c in &mut px[0..3] {
                        *c = acescct::from_linear(acescct::to_linear(*c) * factor);
                    }
                }
                PointwiseOp::Saturation(saturation) => {
                    let lin = [
                        acescct::to_linear(px[0]),
                        acescct::to_linear(px[1]),
                        acescct::to_linear(px[2]),
                    ];
                    let y = acescct::luma(lin[0], lin[1], lin[2]);
                    for i in 0..3 {
                        px[i] = acescct::from_linear(y + saturation * (lin[i] - y));
                    }
                }
            }
        }
    }
}

pub(super) fn mix(base: &[f32], branches: &[&[f32]], output: &mut [f32]) -> anyhow::Result<()> {
    anyhow::ensure!(branches.len() >= 2, "Mix requires at least two branches");
    anyhow::ensure!(
        output.len() % 4 == 0
            && base.len() == output.len()
            && branches.iter().all(|b| b.len() == output.len()),
        "mix buffers must have the same length"
    );

    for (i, out_px) in output.chunks_exact_mut(4).enumerate() {
        let index = i * 4;
        out_px[0] = base[index];
        out_px[1] = base[index + 1];
        out_px[2] = base[index + 2];
        out_px[3] = base[index + 3];
        for branch in branches {
            out_px[0] += branch[index] - base[index];
            out_px[1] += branch[index + 1] - base[index + 1];
            out_px[2] += branch[index + 2] - base[index + 2];
        }
    }

    Ok(())
}

#[inline]
fn blend_channel(background: f32, foreground: f32, mode: BlendMode) -> f32 {
    match mode {
        BlendMode::Normal => foreground,
        BlendMode::Add => background + foreground,
        BlendMode::Subtract => background - foreground,
        BlendMode::Multiply => background * foreground,
        BlendMode::Screen => 1.0 - (1.0 - background) * (1.0 - foreground),
        BlendMode::Overlay => {
            if background <= 0.5 {
                2.0 * background * foreground
            } else {
                1.0 - 2.0 * (1.0 - background) * (1.0 - foreground)
            }
        }
        BlendMode::Difference => (background - foreground).abs(),
        BlendMode::Darken => background.min(foreground),
        BlendMode::Lighten => background.max(foreground),
    }
}

/// Composite straight-alpha `foreground` over `background` using `mode`.
pub(super) fn merge(
    foreground: &[f32],
    background: &[f32],
    mode: BlendMode,
    to: &mut [f32],
) -> anyhow::Result<()> {
    for ((output_px, background_px), foreground_px) in to
        .chunks_exact_mut(4)
        .zip(background.chunks_exact(4))
        .zip(foreground.chunks_exact(4))
    {
        let background_alpha = background_px[3].clamp(0.0, 1.0);
        let foreground_alpha = foreground_px[3].clamp(0.0, 1.0);
        let output_alpha = foreground_alpha + background_alpha * (1.0 - foreground_alpha);

        for channel in 0..3 {
            let background_color = background_px[channel];
            let foreground_color = foreground_px[channel];
            let blended = blend_channel(background_color, foreground_color, mode);
            let premultiplied = background_color * background_alpha * (1.0 - foreground_alpha)
                + foreground_color * foreground_alpha * (1.0 - background_alpha)
                + blended * foreground_alpha * background_alpha;

            output_px[channel] = if output_alpha > f32::EPSILON {
                premultiplied / output_alpha
            } else {
                0.0
            };
        }
        output_px[3] = output_alpha;
    }
    Ok(())
}

pub(super) fn histogram(
    from: &CPUFrame,
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
            (x / 1.0 * (width - 1) as f32)
                .round()
                .clamp(0.0, (width - 1) as f32) as usize
        };
        let y = to_slot(display_p3::luma(px[0], px[1], px[2]));
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
    let mut rgb_buffer = CPUFrame::new(width, height);
    let mut y_buffer = CPUFrame::new(width, height);
    rgb_buffer.set_alpha();
    y_buffer.set_alpha();

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
                rgb_buffer.rgba[index + 0] = 1.0;
            }
        });
        g_hist.iter().enumerate().for_each(|(x, bar_height)| {
            for row in 0..*bar_height {
                let y = height - 1 - row;
                let index = 4 * (y * width + x);
                rgb_buffer.rgba[index + 1] = 1.0;
            }
        });
        b_hist.iter().enumerate().for_each(|(x, bar_height)| {
            for row in 0..*bar_height {
                let y = height - 1 - row;
                let index = 4 * (y * width + x);
                rgb_buffer.rgba[index + 2] = 1.0;
            }
        });
    }
    if y_max_height != 0 {
        y_hist.iter().enumerate().for_each(|(x, bar_height)| {
            for row in 0..*bar_height {
                let y = height - 1 - row;
                let index = 4 * (y * width + x);
                y_buffer.rgba[index + 0] = 1.0;
                y_buffer.rgba[index + 1] = 1.0;
                y_buffer.rgba[index + 2] = 1.0;
            }
        });
    }
    let y_rendered = {
        let mut rendered = RenderedFrame::new(y_buffer.width, y_buffer.height);
        render_frame(&y_buffer.rgba, false, &mut rendered.bgra, 0);
        rendered
    };
    let rgb_rendered = {
        let mut rendered = RenderedFrame::new(rgb_buffer.width, rgb_buffer.height);
        render_frame(&rgb_buffer.rgba, false, &mut rendered.bgra, 0);
        rendered
    };
    (y_rendered, rgb_rendered)
}

pub(super) fn waveform(from: &CPUFrame, width: usize, height: usize) -> RenderedFrame {
    assert!(width > 0 && height > 0);
    // TODO: Support RGB waveform
    // Analyze
    let mut density = vec![0u32; width * height];
    for row in from.rgba.chunks_exact(4 * from.width) {
        for (x, px) in row.chunks_exact(4).enumerate() {
            let y = display_p3::luma(px[0], px[1], px[2]);
            let scope_x = x * width / from.width;
            let normalized_y = (y / 1.0).clamp(0.0, 1.0);
            let scope_y = ((1.0 - normalized_y) * (height - 1) as f32).round() as usize;
            density[scope_y * width + scope_x] += 1;
        }
    }
    // Draw
    let max_density = density.iter().copied().max().unwrap_or(0);
    let mut buffer = CPUFrame::new(width, height);
    buffer.set_alpha();
    if max_density == 0 {
        let mut rendered = RenderedFrame::new(buffer.width, buffer.height);
        render_frame(&buffer.rgba, false, &mut rendered.bgra, 0);
        return rendered;
    }
    for (y, row) in density.chunks_exact(width).enumerate() {
        for (x, p) in row.iter().enumerate() {
            let density = (1.0 + *p as f32).ln() / (1.0 + max_density as f32).ln();
            let index = (y * width + x) * 4;
            buffer.rgba[index] = density;
            buffer.rgba[index + 1] = density;
            buffer.rgba[index + 2] = density;
        }
    }

    let mut rendered = RenderedFrame::new(buffer.width, buffer.height);
    render_frame(&buffer.rgba, false, &mut rendered.bgra, 0);
    return rendered;
}

pub(super) fn vectorscope(from: &CPUFrame, width: usize, height: usize) -> RenderedFrame {
    assert!(width > 0 && height > 0);
    // Analyze
    let size = width.min(height);
    let radius = (size - 1) as f32 / 2.0;
    let center_x = (width - 1) as f32 / 2.0;
    let center_y = (height - 1) as f32 / 2.0;
    let mut density = vec![0u32; width * height];
    for row in from.rgba.chunks_exact(4 * from.width) {
        for px in row.chunks_exact(4) {
            let (cb, cr) = display_p3::to_cb_cr(px[0], px[1], px[2]);
            let x = center_x + cb * radius;
            let y = center_y - cr * radius;
            let x = x.round().clamp(0.0, (width - 1) as f32) as usize;
            let y = y.round().clamp(0.0, (height - 1) as f32) as usize;
            density[y * width + x] += 1;
        }
    }
    // Draw
    let mut buffer = CPUFrame::new(width, height);
    buffer.set_alpha();
    let max_density = density.iter().copied().max().unwrap_or(0);
    if max_density == 0 {
        let mut rendered = RenderedFrame::new(buffer.width, buffer.height);
        render_frame(&buffer.rgba, false, &mut rendered.bgra, 0);
        return rendered;
    }
    for (py, row) in density.chunks_exact(width).enumerate() {
        for (px, p) in row.iter().enumerate() {
            let cb = (px as f32 - center_x) / radius;
            let cr = (center_y - py as f32) / radius;
            let (r, g, b) = display_p3::from_cb_cr(0.5, cb, cr);
            let (r, g, b) = (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0));
            // Improve brightness for readability
            let max_channel = r.max(g).max(b);
            let scale = if max_channel > f32::EPSILON {
                1.0 / max_channel
            } else {
                0.0
            };
            let density = (1.0 + *p as f32).ln() / (1.0 + max_density as f32).ln();
            let index = (py * width + px) * 4;
            buffer.rgba[index] = r * density * scale;
            buffer.rgba[index + 1] = g * density * scale;
            buffer.rgba[index + 2] = b * density * scale;
        }
    }
    let mut rendered = RenderedFrame::new(buffer.width, buffer.height);
    render_frame(&buffer.rgba, false, &mut rendered.bgra, 0);
    rendered
}

/// Rendered a internal ScalarFrame into a RenderedFrame for screen.
/// Since RenderedFrame uses 8 bit, we need to add some noise.
pub(super) fn render_frame(frame: &[f32], add_noise: bool, to: &mut [u8], index_offset: usize) {
    #[inline(always)]
    fn convert(point: f32, noise: f32) -> u8 {
        (point.clamp(0.0, 1.0) * 255.0 + noise)
            .round()
            .clamp(0.0, 255.0) as u8
    }
    if add_noise {
        for (index, px) in frame.chunks_exact(4).enumerate() {
            let noise = tpdf_noise((index + index_offset) as u64);
            to[index * 4] = convert(px[2], noise); // B
            to[index * 4 + 1] = convert(px[1], noise); // G
            to[index * 4 + 2] = convert(px[0], noise); // R
            to[index * 4 + 3] = convert(px[3], noise); // A
        }
    } else {
        for (index, px) in frame.chunks_exact(4).enumerate() {
            to[index * 4] = convert(px[2], 0.0); // B
            to[index * 4 + 1] = convert(px[1], 0.0); // G
            to[index * 4 + 2] = convert(px[0], 0.0); // R
            to[index * 4 + 3] = convert(px[3], 0.0); // A
        }
    }
}

#[inline(always)]
fn tpdf_noise(key: u64) -> f32 {
    let hash = xxh3_64(&key.to_le_bytes());

    let u1 = (hash & 0xffff) as f32 * (1.0 / 65536.0);
    let u2 = ((hash >> 16) & 0xffff) as f32 * (1.0 / 65536.0);
    u1 + u2 - 1.0
}

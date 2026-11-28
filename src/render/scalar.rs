use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, bail};
use ocio_rs::transform::{
    BuiltinTransform, ColorSpaceTransform, FileTransform, GroupTransform, Transform,
};
use ocio_rs::{Config, ConfigIOProxy, StaticCPUProcessor, TransformDirection};

use super::render_backend::RenderBackend;
use super::render_backend::ScopeGraph;
use super::render_list::{Op, RenderList, Slot};
use super::{BackendFrame, DecodedImage, MediaIcc, RenderedFrame};

/// The most basic render backend. It deliberately favors correctness over
/// performance and serves as the reference implementation for regression tests
/// against optimized backends.
pub struct ScalarBackend {
    input_processor_cache: Mutex<HashMap<u128, Arc<StaticCPUProcessor>>>,
    fallback_input_processor: Arc<StaticCPUProcessor>,
    output_transform: StaticCPUProcessor,
}

impl ScalarBackend {
    pub fn new() -> Self {
        let cfg =
            Config::from_file("ocio://cg-config-latest").expect("Load OCIO config should not fail");

        // Images without an ICC profile are interpreted as display-referred sRGB
        // sRGB - Display -> CIE XYZ_D65 - Display
        //                -> (see as) CIE-XYZ-D65 - SDR-100nit-P3-D65_2.0
        //                -> ACES2065-1
        //                -> ACEScct
        let fallback_input_processor = {
            let group = GroupTransform::create().expect("create display input group");

            let to_xyz = ColorSpaceTransform::create().expect("create display color transform");
            to_xyz
                .set_src("sRGB - Display")
                .expect("set display source");
            to_xyz
                .set_dst("CIE XYZ-D65 - Display-referred")
                .expect("set display-reference XYZ destination");
            group
                .append_transform(&to_xyz)
                .expect("append display color transform");

            let inverse_view = BuiltinTransform::create().expect("create inverse view transform");
            inverse_view
                .set_style("ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - SDR-100nit-P3-D65_2.0")
                .expect("set inverse view style");
            inverse_view
                .try_set_direction(TransformDirection::Inverse)
                .expect("set inverse view direction");
            group
                .append_transform(&inverse_view)
                .expect("append inverse view transform");

            let to_cct = ColorSpaceTransform::create().expect("create ACEScct transform");
            to_cct.set_src("ACES2065-1").expect("set ACES2065-1 source");
            to_cct.set_dst("ACEScct").expect("set ACEScct destination");
            group
                .append_transform(&to_cct)
                .expect("append ACEScct transform");

            cfg.processor_from_transform(&Transform::Group(group), TransformDirection::Forward)
                .expect("create display-to-ACEScct processor")
                .default_cpu_processor()
                .expect("allocate display-to-ACEScct processor")
                .try_into_static()
                .expect("display-to-ACEScct processor must be static")
        };

        let output_transform = cfg
            .processor_display(
                "ACEScct",
                // Keep this in sync with the main window's `use_display_p3`
                // setting.
                "Display P3 - Display",
                "ACES 2.0 - SDR 100 nits (P3 D65)",
                TransformDirection::Forward,
            )
            .expect("Create display processor should not fail")
            .default_cpu_processor()
            .expect("Allocate CPU processor should not fail")
            .try_into_static()
            .expect("Processor should not contain dynamic properties");

        Self {
            input_processor_cache: Mutex::new(HashMap::new()),
            fallback_input_processor: Arc::new(fallback_input_processor),
            output_transform,
        }
    }

    fn get_input_processor(
        &self,
        icc: &Option<MediaIcc>,
    ) -> anyhow::Result<Arc<StaticCPUProcessor>> {
        let Some(icc) = icc else {
            return Ok(self.fallback_input_processor.clone());
        };
        let key = icc.hash();
        if let Some(processor) = self
            .input_processor_cache
            .lock()
            .expect("Input processor cache mutex poisoned")
            .get(&key)
            .cloned()
        {
            return Ok(processor);
        }

        // Add a file proxy to bypass OCIO requiring a icc file on disk
        let path = format!("/not/exists/icc-{key}.icc");

        let proxy = ConfigIOProxy::create().expect("Create Config Proxy should not fail");
        let success = proxy
            .set_lut_data_with_hash_bytes(&path, &icc.raw, &key.to_le_bytes())
            .expect("Insert lut data should not fail");
        assert!(success, "Proxy should accept lut data");

        // Create processor
        let config =
            Config::from_file("ocio://cg-config-latest").expect("Load OCIO config should not fail");
        config
            .set_config_io_proxy_object(&proxy)
            .expect("Insert proxy should not fail");
        let group = GroupTransform::create().expect("Create group transform should not fail");

        // ICC -> CIE XYZ-D65
        let file = FileTransform::create().expect("Create file transform should not failed");
        file.set_src(&path).expect("Set source should not fail");
        file.set_direction(TransformDirection::Inverse);
        group
            .append_transform(&file)
            .expect("Append file transform should not fail");

        if icc.is_display_referred() {
            // CIE XYZ-D65 (see as Display Referred)
            //      -> ACES2065-1
            //      -> ACEScct
            let inverse_view = BuiltinTransform::create().expect("create inverse view transform");
            inverse_view
                .set_style("ACES-OUTPUT - ACES2065-1_to_CIE-XYZ-D65 - SDR-100nit-P3-D65_2.0")
                .expect("set inverse view style");
            inverse_view
                .try_set_direction(TransformDirection::Inverse)
                .expect("set inverse view direction");
            group
                .append_transform(&inverse_view)
                .expect("append inverse view transform");

            let to_cct = ColorSpaceTransform::create().expect("create ACEScct transform");
            to_cct.set_src("ACES2065-1").expect("set ACES2065-1 source");
            to_cct.set_dst("ACEScct").expect("set ACEScct destination");
            group
                .append_transform(&to_cct)
                .expect("append ACEScct transform");
        } else {
            // CIE XYZ-D65 (see as Scene Referred)
            //      -> ACEScct
            let to_cct = ColorSpaceTransform::create().expect("create scene color transform");
            to_cct
                .set_src("CIE XYZ-D65 - Scene-referred")
                .expect("set scene source");
            to_cct.set_dst("ACEScct").expect("set ACEScct destination");
            group
                .append_transform(&to_cct)
                .expect("append scene color transform");
        }

        let processor = config
            .processor_from_transform(&Transform::Group(group), TransformDirection::Forward)
            .context("Failed to create processor from icc")?;
        let cpu = Arc::new(
            processor
                .default_cpu_processor()
                .expect("Allocate CPU processor should not failed")
                .try_into_static()
                .expect("CPU processor should not contain dynamic properties"),
        );
        self.input_processor_cache
            .lock()
            .expect("Input processor cache mutex poisoned")
            .insert(key, cpu.clone());
        Ok(cpu)
    }

    fn execute(&self, list: RenderList, from: &DecodedImage) -> anyhow::Result<ScalarFrame> {
        list.verify().context("invalid render list")?;
        let RenderList {
            slot_count,
            ops,
            input,
            output,
        } = list;

        // Slots are stored as `Option<ScalarFrame>` so that reading a slot that was
        // never written is reported instead of silently producing garbage.
        let mut slots: Vec<Option<ScalarFrame>> = vec![None; slot_count];

        // Prepare input
        let input_index = input.id() as usize;
        let seed = slots.get_mut(input_index).with_context(|| {
            format!("input slot {input_index} out of range (slot_count = {slot_count})")
        })?;
        let mut input = ScalarFrame::from_frame(from);
        let pixel_num = input.rgba.len() / 4;
        let input_transform = self.get_input_processor(&from.icc).unwrap_or_else(|e| {
            error!("{e:#}");
            self.fallback_input_processor.clone()
        });
        input_transform.apply_rgba_pixels(&mut input.rgba, pixel_num as i64, 4);
        *seed = Some(input);

        // Execute body
        for (op_index, op) in ops.iter().enumerate() {
            match op {
                Op::Copy { from, to } => {
                    assert_ne!(from, to);
                    let value = read_slot(&slots, *from, op_index, "from")?.clone();
                    write_slot(&mut slots, *to, value, op_index)?;
                }
                Op::Pointwise {
                    from,
                    to,
                    operations,
                } => {
                    if from == to {
                        kernel::apply_pointwise(
                            slot_mut(&mut slots, *to, op_index, "to")?,
                            operations,
                        );
                    } else {
                        let mut value = read_slot(&slots, *from, op_index, "from")?.clone();
                        kernel::apply_pointwise(&mut value, operations);
                        write_slot(&mut slots, *to, value, op_index)?;
                    }
                }
                Op::Merge { fg, bg, to, mode } => {
                    let foreground = read_slot(&slots, *fg, op_index, "fg")?;
                    let background = read_slot(&slots, *bg, op_index, "bg")?;
                    let value = kernel::merge(foreground, background, *mode)?;
                    write_slot(&mut slots, *to, value, op_index)?;
                }
                Op::Mix { base, from, to } => {
                    let base = read_slot(&slots, *base, op_index, "base")?;
                    let branches = from
                        .iter()
                        .map(|slot| read_slot(&slots, *slot, op_index, "from"))
                        .collect::<anyhow::Result<Vec<_>>>()?;

                    let value = kernel::mix(base, &branches)?;

                    write_slot(&mut slots, *to, value, op_index)?;
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
                let pixel_num = frame.rgba.len() / 4;
                self.output_transform
                    .apply_rgba_pixels(&mut frame.rgba, pixel_num as i64, 4);
                frame
            })
    }
}

/// Read a slot, failing if it is out of range or was never written.
fn read_slot<'a>(
    slots: &'a [Option<ScalarFrame>],
    slot: Slot,
    op_index: usize,
    name: &str,
) -> anyhow::Result<&'a ScalarFrame> {
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
    slots: &'a mut [Option<ScalarFrame>],
    slot: Slot,
    op_index: usize,
    name: &str,
) -> anyhow::Result<&'a mut ScalarFrame> {
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
    slots: &mut [Option<ScalarFrame>],
    slot: Slot,
    frame: ScalarFrame,
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
        let rendered = kernel::render_frame(&buffer, true);
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

    fn render_thumbnail(&self, from: Arc<DecodedImage>) -> Arc<RenderedFrame> {
        let mut buffer = ScalarFrame::from_frame(&from);
        let pixel_num = buffer.rgba.len() / 4;
        let input_transform = self.get_input_processor(&from.icc).unwrap_or_else(|e| {
            error!("{e:#}");
            self.fallback_input_processor.clone()
        });
        input_transform.apply_rgba_pixels(&mut buffer.rgba, pixel_num as i64, 4);
        self.output_transform
            .apply_rgba_pixels(&mut buffer.rgba, pixel_num as i64, 4);
        let rendered = kernel::render_frame(&buffer, true);
        Arc::new(rendered)
    }
}

/// Intermediate frame representation used between processing steps.
#[derive(Clone, Debug)]
struct ScalarFrame {
    width: usize,
    height: usize,
    rgba: Vec<f32>,
}

impl ScalarFrame {
    fn new(width: usize, height: usize) -> Self {
        let mut rgba = vec![0.0f32; width * height * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 1.0; // let alpha = 1
        }
        ScalarFrame {
            width,
            height,
            rgba: rgba,
        }
    }
    /// Convert to f32 and normalize
    fn from_frame(frame: &DecodedImage) -> Self {
        ScalarFrame {
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

mod kernel {
    use xxhash_rust::xxh3::xxh3_64;

    use super::ScalarFrame;
    use crate::render::{BlendMode, PointwiseOp, RenderedFrame};

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

    pub(super) fn apply_pointwise(buffer: &mut ScalarFrame, operations: &[PointwiseOp]) {
        for px in buffer.rgba.chunks_exact_mut(4) {
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

    pub(super) fn mix(
        base: &ScalarFrame,
        branches: &[&ScalarFrame],
    ) -> anyhow::Result<ScalarFrame> {
        anyhow::ensure!(branches.len() >= 2, "Mix requires at least two branches");

        for branch in branches {
            anyhow::ensure!(
                branch.width == base.width && branch.height == base.height,
                "cannot mix frames of different sizes: \
             base is {}x{}, branch is {}x{}",
                base.width,
                base.height,
                branch.width,
                branch.height,
            );
        }

        let mut output = base.clone();

        for branch in branches {
            for ((output_px, base_px), branch_px) in output
                .rgba
                .chunks_exact_mut(4)
                .zip(base.rgba.chunks_exact(4))
                .zip(branch.rgba.chunks_exact(4))
            {
                output_px[0] += branch_px[0] - base_px[0];
                output_px[1] += branch_px[1] - base_px[1];
                output_px[2] += branch_px[2] - base_px[2];

                // TODO: Support mix alpha channel
                output_px[3] = base_px[3];
            }
        }

        Ok(output)
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
        foreground: &ScalarFrame,
        background: &ScalarFrame,
        mode: BlendMode,
    ) -> anyhow::Result<ScalarFrame> {
        anyhow::ensure!(
            foreground.width == background.width && foreground.height == background.height,
            "cannot merge frames of different sizes: {}x{} and {}x{}",
            foreground.width,
            foreground.height,
            background.width,
            background.height
        );

        let mut output = ScalarFrame::new(background.width, background.height);
        for ((output_px, background_px), foreground_px) in output
            .rgba
            .chunks_exact_mut(4)
            .zip(background.rgba.chunks_exact(4))
            .zip(foreground.rgba.chunks_exact(4))
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
        Ok(output)
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
        (
            render_frame(&y_buffer, false),
            render_frame(&rgb_buffer, false),
        )
    }

    pub(super) fn waveform(from: &ScalarFrame, width: usize, height: usize) -> RenderedFrame {
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
        let mut buffer = ScalarFrame::new(width, height);
        if max_density == 0 {
            return render_frame(&buffer, false);
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

        render_frame(&buffer, false)
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
                let (cb, cr) = display_p3::to_cb_cr(px[0], px[1], px[2]);
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
            return render_frame(&buffer, false);
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

        render_frame(&buffer, false)
    }

    /// Rendered a internal ScalarFrame into a RenderedFrame for screen.
    /// Since RenderedFrame uses 8 bit, we need to add some noise.
    pub(super) fn render_frame(frame: &ScalarFrame, add_noise: bool) -> RenderedFrame {
        let mut buffer = vec![0u8; frame.width * frame.height * 4];
        #[inline(always)]
        fn convert(point: f32, noise: f32) -> u8 {
            (point.clamp(0.0, 1.0) * 255.0 + noise)
                .round()
                .clamp(0.0, 255.0) as u8
        }
        if add_noise {
            for (index, px) in frame.rgba.chunks_exact(4).enumerate() {
                let noise = tpdf_noise(index as u64);
                buffer[index * 4] = convert(px[2], noise); // B
                buffer[index * 4 + 1] = convert(px[1], noise); // G
                buffer[index * 4 + 2] = convert(px[0], noise); // R
                buffer[index * 4 + 3] = convert(px[3], noise); // A
            }
        } else {
            for (index, px) in frame.rgba.chunks_exact(4).enumerate() {
                buffer[index * 4] = convert(px[2], 0.0); // B
                buffer[index * 4 + 1] = convert(px[1], 0.0); // G
                buffer[index * 4 + 2] = convert(px[0], 0.0); // R
                buffer[index * 4 + 3] = convert(px[3], 0.0); // A
            }
        }
        RenderedFrame {
            width: frame.width,
            height: frame.height,
            bgra: buffer,
        }
    }

    #[inline(always)]
    fn tpdf_noise(key: u64) -> f32 {
        let hash = xxh3_64(&key.to_le_bytes());

        let u1 = (hash & 0xffff) as f32 * (1.0 / 65536.0);
        let u2 = ((hash >> 16) & 0xffff) as f32 * (1.0 / 65536.0);
        u1 + u2 - 1.0
    }
}

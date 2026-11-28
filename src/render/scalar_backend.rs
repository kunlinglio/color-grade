use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, bail};
use ocio_rs::transform::{
    BuiltinTransform, ColorSpaceTransform, FileTransform, GroupTransform, Transform,
};
use ocio_rs::{Config, ConfigIOProxy, StaticCPUProcessor, TransformDirection};

use super::cpu_kernel::{self};
use super::render_backend::RenderBackend;
use super::render_backend::ScopeGraph;
use super::render_list::{Op, RenderList, Slot};
use super::{BackendFrame, DecodedImage, MediaIcc, RenderedFrame};

/// The most basic render backend. It deliberately favors prioritize over
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

    fn execute(
        &self,
        list: RenderList,
        from: &DecodedImage,
    ) -> anyhow::Result<cpu_kernel::CpuFrame> {
        list.verify().context("invalid render list")?;
        let RenderList {
            slot_count,
            ops,
            input,
            output,
        } = list;

        // Slots are stored as `Option<ScalarFrame>` so that reading a slot that was
        // never written is reported instead of silently producing garbage.
        let mut slots: Vec<Option<cpu_kernel::CpuFrame>> = vec![None; slot_count];

        // Prepare input
        let input_index = input.id() as usize;
        let seed = slots.get_mut(input_index).with_context(|| {
            format!("input slot {input_index} out of range (slot_count = {slot_count})")
        })?;
        let mut input = cpu_kernel::CpuFrame::from_frame(from);
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
                        cpu_kernel::apply_pointwise(
                            slot_mut(&mut slots, *to, op_index, "to")?,
                            operations,
                        );
                    } else {
                        let mut value = read_slot(&slots, *from, op_index, "from")?.clone();
                        cpu_kernel::apply_pointwise(&mut value, operations);
                        write_slot(&mut slots, *to, value, op_index)?;
                    }
                }
                Op::Merge { fg, bg, to, mode } => {
                    let foreground = read_slot(&slots, *fg, op_index, "fg")?;
                    let background = read_slot(&slots, *bg, op_index, "bg")?;
                    let value = cpu_kernel::merge(foreground, background, *mode)?;
                    write_slot(&mut slots, *to, value, op_index)?;
                }
                Op::Mix { base, from, to } => {
                    let base = read_slot(&slots, *base, op_index, "base")?;
                    let branches = from
                        .iter()
                        .map(|slot| read_slot(&slots, *slot, op_index, "from"))
                        .collect::<anyhow::Result<Vec<_>>>()?;

                    let value = cpu_kernel::mix(base, &branches)?;

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
    slots: &'a [Option<cpu_kernel::CpuFrame>],
    slot: Slot,
    op_index: usize,
    name: &str,
) -> anyhow::Result<&'a cpu_kernel::CpuFrame> {
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
    slots: &'a mut [Option<cpu_kernel::CpuFrame>],
    slot: Slot,
    op_index: usize,
    name: &str,
) -> anyhow::Result<&'a mut cpu_kernel::CpuFrame> {
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
    slots: &mut [Option<cpu_kernel::CpuFrame>],
    slot: Slot,
    frame: cpu_kernel::CpuFrame,
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
        let rendered = cpu_kernel::render_frame(&buffer, true);
        (BackendFrame::new(buffer), Arc::new(rendered))
    }

    fn handle_scope(&self, from: BackendFrame, width: usize, height: usize) -> ScopeGraph {
        let frame = from.downcast::<cpu_kernel::CpuFrame>();
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
        let mut buffer = cpu_kernel::CpuFrame::from_frame(&from);
        let pixel_num = buffer.rgba.len() / 4;
        let input_transform = self.get_input_processor(&from.icc).unwrap_or_else(|e| {
            error!("{e:#}");
            self.fallback_input_processor.clone()
        });
        input_transform.apply_rgba_pixels(&mut buffer.rgba, pixel_num as i64, 4);
        self.output_transform
            .apply_rgba_pixels(&mut buffer.rgba, pixel_num as i64, 4);
        let rendered = cpu_kernel::render_frame(&buffer, true);
        Arc::new(rendered)
    }
}

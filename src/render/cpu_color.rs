use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use ocio_rs::transform::{
    BuiltinTransform, ColorSpaceTransform, FileTransform, GroupTransform, Transform,
};
use ocio_rs::{Config, ConfigIOProxy, StaticCPUProcessor, TransformDirection};

use super::MediaIcc;

pub(super) struct CPUColorPipeline {
    input_processor_cache: Mutex<HashMap<u128, Arc<StaticCPUProcessor>>>,
    fallback_input_processor: Arc<StaticCPUProcessor>,
    output_transform: StaticCPUProcessor,
}

impl CPUColorPipeline {
    pub(super) fn new() -> Self {
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

    pub(super) fn apply_input(&self, rgba: &mut [f32], icc: &Option<MediaIcc>) {
        let processor = self.get_input_processor(icc).unwrap_or_else(|e| {
            error!("{e:#}");
            self.fallback_input_processor.clone()
        });
        processor.apply_rgba_pixels(rgba, (rgba.len() / 4) as i64, 4);
    }

    pub(super) fn apply_output(&self, rgba: &mut [f32]) {
        self.output_transform
            .apply_rgba_pixels(rgba, (rgba.len() / 4) as i64, 4);
    }
}

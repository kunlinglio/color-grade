mod scheduler;
use rayon::ThreadPool;
pub use scheduler::{FrameStream, Scheduler};
mod render_list;
pub use render_list::{BlendMode, Op, PointwiseOp, RenderList, Slot};
mod render_backend;
pub use render_backend::{BackendFrame, RenderBackend, ScopeGraph};
mod cpu_backend;
mod cpu_color;
mod cpu_kernel;
mod scalar_backend;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use image::{DynamicImage, ImageDecoder, RgbaImage};

pub fn new_backend(thread_pool: Arc<ThreadPool>) -> Arc<dyn RenderBackend> {
    if cfg!(feature = "backend-cpu") {
        Arc::new(cpu_backend::CPUBackend::new(thread_pool))
    } else {
        let _ = thread_pool;
        Arc::new(scalar_backend::ScalarBackend::new())
    }
}

/// A Frame only for UI usage.
/// Following GPUI requirement, we use bgra inside.
pub struct RenderedFrame {
    width: usize,
    height: usize,
    bgra: Vec<u8>,
}

impl RenderedFrame {
    pub(super) fn new(width: usize, height: usize) -> Self {
        let bgra = vec![0u8; width * height * 4];
        RenderedFrame {
            width,
            height,
            bgra,
        }
    }
    pub fn to_image(&self) -> RgbaImage {
        image::RgbaImage::from_raw(self.width as u32, self.height as u32, self.bgra.clone())
            .unwrap()
    }
    /// Return (width, height)
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

/// A decoded image for cache and transferring.
// #[derive(Clone)]
pub struct DecodedImage {
    width: usize,
    height: usize,
    /// We stored decoded image using 16bit integer for now.
    /// Sinc raw is not supported and 16bit is enough for
    /// common image format.
    rgba: Vec<u16>,
    icc: Option<MediaIcc>,
}
impl DecodedImage {
    pub fn decode_from(path: &PathBuf) -> anyhow::Result<Self> {
        let mut decoder = image::ImageReader::open(&path)
            .with_context(|| format!("Failed to open image {}", path.display()))?
            .with_guessed_format()
            .with_context(|| format!("Failed to guess format for {}", path.display()))?
            .into_decoder()
            .with_context(|| format!("Failed to decode image {}", path.display()))?;
        // TODO: Convert icc into OCIO supported format
        let icc = MediaIcc::from_image(&mut decoder)
            .with_context(|| format!("Failed to read icc profile for {}", path.display()))?;
        let img = DynamicImage::from_decoder(decoder)?;
        let rgba = img.to_rgba16();
        let (width, height) = rgba.dimensions();
        Ok(DecodedImage {
            width: width as usize,
            height: height as usize,
            rgba: rgba.to_vec(),
            icc,
        })
    }
    pub fn decode_thumb_from(
        path: &PathBuf,
        max_width: u32,
        max_height: u32,
    ) -> anyhow::Result<Self> {
        let mut decoder = image::ImageReader::open(&path)
            .with_context(|| format!("Failed to open image {}", path.display()))?
            .with_guessed_format()
            .with_context(|| format!("Failed to guess format for {}", path.display()))?
            .into_decoder()
            .with_context(|| format!("Failed to decode image {}", path.display()))?;
        // TODO: Convert icc into OCIO supported format
        let icc = MediaIcc::from_image(&mut decoder)
            .with_context(|| format!("Failed to read icc profile for {}", path.display()))?;
        let img = DynamicImage::from_decoder(decoder)?;
        let img = img.thumbnail(max_width, max_height);
        let rgba = img.to_rgba16();
        let (width, height) = rgba.dimensions();
        Ok(DecodedImage {
            width: width as usize,
            height: height as usize,
            rgba: rgba.to_vec(),
            icc,
        })
    }
    /// Return (width, height)
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

struct MediaIcc {
    raw: Vec<u8>,
    device_class: lcms2::ProfileClassSignature,
}

impl MediaIcc {
    pub fn from_image(image: &mut impl ImageDecoder) -> anyhow::Result<Option<Self>> {
        let raw = image.icc_profile().context("Failed to read icc profile")?;
        raw.map(Self::from_raw).transpose()
    }

    fn from_raw(raw: Vec<u8>) -> anyhow::Result<Self> {
        let profile = lcms2::Profile::new_icc(&raw).context("Failed to parse icc bytes")?;
        Ok(Self {
            device_class: profile.device_class(),
            raw,
        })
    }

    fn is_display_referred(&self) -> bool {
        self.device_class == lcms2::ProfileClassSignature::DisplayClass
    }

    fn hash(&self) -> u128 {
        let mut hasher = xxhash_rust::xxh3::Xxh3::new();
        for slice in self.raw.chunks(64 * 1024) {
            hasher.update(slice);
        }
        hasher.digest128()
    }
}

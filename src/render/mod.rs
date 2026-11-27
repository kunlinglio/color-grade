mod scheduler;
use anyhow::Context;
pub use scheduler::FrameStream;
mod render_list;
pub use render_list::{BlendMode, Op, PointwiseOp, RenderList, Slot};
mod renderer;
pub use renderer::{BackendFrame, Renderer, ScopeGraph};
mod scalar;

use image::{ImageDecoder, RgbaImage};

/// A Frame only for UI usage.
/// Following GPUI requirement, we use bgra inside.
pub struct RenderedFrame {
    width: usize,
    height: usize,
    bgra: Vec<u8>,
}

impl RenderedFrame {
    pub fn from_rgba(width: usize, height: usize, rgba: Vec<u8>) -> Self {
        assert_eq!(width * height * 4, rgba.len(), "RGBA buffer size mismatch");
        let mut bgra = rgba;
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
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
    pub fn from_frame(frame: Frame) -> Self {
        Self::from_rgba(frame.width, frame.height, frame.rgba)
    }
    /// Return (width, height)
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

/// A frame buffer for transferring buffers.
/// Use rgba to make handling easier.
#[derive(Clone)]
pub struct Frame {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}
impl Frame {
    pub fn new(width: usize, height: usize, rgba: Vec<u8>) -> Self {
        Frame {
            width,
            height,
            rgba,
        }
    }
    /// Return (width, height)
    pub fn _size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

pub struct MediaIcc {
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

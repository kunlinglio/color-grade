pub mod comm;
pub mod render_list;
pub mod renderer;

use image::RgbaImage;

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
    pub fn _size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

/// A frame buffer for transferring buffers.
/// Use rgba to make handling easier.
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

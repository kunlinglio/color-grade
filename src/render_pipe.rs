pub trait RenderPipe {
    fn size(&self) -> (usize, usize);
    fn render(&self, buffer: &mut [u8]);
}

pub struct CPURenderPipe {
    width: usize,
    height: usize,
    fill: [u8; 4], // RGBA
}

impl CPURenderPipe {
    pub fn new(width: usize, height: usize, fill: [u8; 4]) -> Self {
        Self {
            width,
            height,
            fill,
        }
    }
}

impl RenderPipe for CPURenderPipe {
    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn render(&self, buffer: &mut [u8]) {
        assert_eq!(buffer.len(), self.width * self.height * 4);
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.copy_from_slice(&self.fill);
        }
    }
}

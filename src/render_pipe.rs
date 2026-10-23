pub trait RenderPipe {
    fn rende(&self, buffer: &mut [u8]);
}

pub struct CPURenderPipe {
    origin_picture: Vec<u8>,
}

impl CPURenderPipe {
    pub fn new(picture: Vec<u8>) -> Self {
        CPURenderPipe {
            origin_picture: picture,
        }
    }
}

impl RenderPipe for CPURenderPipe {
    fn rende(&self, buffer: &mut [u8]) {
        assert_eq!(buffer.len(), self.origin_picture.len());
        buffer.copy_from_slice(&self.origin_picture);
    }
}

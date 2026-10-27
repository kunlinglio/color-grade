use std::path::PathBuf;
use std::sync::Arc;

use crate::render::Frame;
use crate::render::render_list::{Op, RenderList};

/// A opened or edited media inside a project.
pub struct Editable {
    workflow: WorkFlow,
    decoded: Arc<Frame>,
}

impl Editable {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let img = image::ImageReader::open(&path)?
            .with_guessed_format()?
            .decode()?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let decoded_frame = Arc::new(Frame::new(width as usize, height as usize, rgba.to_vec()));
        Ok(Editable {
            workflow: WorkFlow {},
            decoded: decoded_frame,
        })
    }

    pub fn gen_render_list(&self) -> RenderList {
        self.workflow.to_render_list()
    }

    pub fn get_decoded(&self) -> Arc<Frame> {
        self.decoded.clone()
    }
}

pub struct WorkFlow {}

impl WorkFlow {
    fn to_render_list(&self) -> RenderList {
        RenderList {
            ops: vec![Op::Brightness(1.05), Op::Saturation(0.8)],
        }
    }
}

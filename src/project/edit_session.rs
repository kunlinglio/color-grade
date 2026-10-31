use std::path::PathBuf;
use std::sync::Arc;

use crate::render::Frame;
use crate::render::{Op, RenderList};

/// A opened or edited media inside a project.
pub struct EditSession {
    workflow: WorkFlow,
    decoded: Arc<Frame>,
}

impl EditSession {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let img = image::ImageReader::open(&path)?
            .with_guessed_format()?
            .decode()?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let decoded_frame = Arc::new(Frame::new(width as usize, height as usize, rgba.to_vec()));
        Ok(EditSession {
            workflow: WorkFlow::default(),
            decoded: decoded_frame,
        })
    }

    pub fn gen_render_list(&self) -> RenderList {
        self.workflow.to_render_list()
    }

    pub fn get_decoded(&self) -> Arc<Frame> {
        self.decoded.clone()
    }

    /// Incrementally update a workflow parameter owned by this session.
    pub fn set_brightness(&mut self, brightness: f32) {
        self.workflow.brightness = brightness;
    }

    pub fn set_saturation(&mut self, saturation: f32) {
        self.workflow.saturation = saturation;
    }
}

/// The editable grade workflow, owned by the runtime's [`EditSession`].
#[derive(Clone)]
pub struct WorkFlow {
    brightness: f32,
    saturation: f32,
}

impl Default for WorkFlow {
    fn default() -> Self {
        WorkFlow {
            brightness: 1.0,
            saturation: 1.0,
        }
    }
}

impl WorkFlow {
    fn to_render_list(&self) -> RenderList {
        RenderList {
            ops: vec![
                Op::Brightness(self.brightness),
                Op::Saturation(self.saturation),
            ],
        }
    }
}

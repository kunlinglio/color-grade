use std::path::PathBuf;
use std::sync::Arc;

use crate::render::Frame;
use crate::render::{Op, RenderList};

/// A opened or edited media inside a project.
pub struct Session {
    workflow: WorkFlow,
    decoded: Arc<Frame>,
}

impl Session {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let img = image::ImageReader::open(&path)?
            .with_guessed_format()?
            .decode()?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let decoded_frame = Arc::new(Frame::new(width as usize, height as usize, rgba.to_vec()));
        Ok(Session {
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

    /// Return whether need to trigger rerender
    pub fn update(&mut self, diff: WorkFlowDiff) -> bool {
        self.workflow.apply_diff(diff)
    }

    pub fn get_workflow(&self) -> WorkFlow {
        self.workflow.clone()
    }
}

/// The editable grade workflow, owned by the runtime's [`EditSession`].
#[derive(Clone, Copy)]
pub struct WorkFlow {
    pub brightness: f32,
    pub saturation: f32,
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
    /// Return whether need to rerender
    fn apply_diff(&mut self, diff: WorkFlowDiff) -> bool {
        match diff {
            WorkFlowDiff::Brightness(new) => self.brightness = new,
            WorkFlowDiff::Saturation(new) => self.saturation = new,
        }
        true
    }
}

#[derive(Clone, Copy)]
pub enum WorkFlowDiff {
    Brightness(f32),
    Saturation(f32),
}

use std::path::PathBuf;
use std::sync::Arc;

use crate::render::{Frame, RenderList};

use super::{WorkFlow, WorkFlowDiff};

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

    pub fn gen_render_list(&self) -> anyhow::Result<RenderList> {
        self.workflow.compile()
    }

    pub fn get_decoded(&self) -> Arc<Frame> {
        self.decoded.clone()
    }

    /// Return whether need to trigger rerender
    pub fn update(&mut self, diff: WorkFlowDiff) -> bool {
        self.workflow.apply_diff(diff)
    }

    pub fn get_workflow(&self) -> WorkFlow {
        //TODO: Optimize this clone
        self.workflow.clone()
    }
}

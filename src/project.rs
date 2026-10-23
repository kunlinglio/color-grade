use std::path::PathBuf;
use std::sync::Mutex;

use crate::render_pipe::{CPURenderPipe, RenderPipe};

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    project_path: PathBuf,
    pub renderer: Mutex<Box<dyn RenderPipe>>,
}

impl ProjectRuntime {
    pub fn new(project_path: PathBuf) -> Self {
        let renderer: Box<dyn RenderPipe> =
            Box::new(CPURenderPipe::new(1920, 1080, [128, 249, 134, 255]));
        let renderer = Mutex::new(renderer);
        ProjectRuntime {
            project_path: project_path,
            renderer,
        }
    }
}

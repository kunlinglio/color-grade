use crate::render_pipe::{CPURenderPipe, RenderPipe};
use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;
use std::boxed::Box;
use std::path::PathBuf;

const IMG_PATH: &str = "./test_photo.jpg";

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    project_path: PathBuf,
    renderer: Box<dyn RenderPipe>,
}

impl ProjectRuntime {
    pub fn new(project_path: PathBuf) -> Self {
        let renderer = Box::new(CPURenderPipe::new(1920, 1080, [128, 249, 134, 255]));
        ProjectRuntime {
            project_path: project_path,
            renderer,
        }
    }
}

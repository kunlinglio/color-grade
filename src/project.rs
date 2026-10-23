use crate::render_pipe::RenderPipe;
use std::boxed::Box;
use std::path::PathBuf;

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    project_path: PathBuf,
    renderer: Box<dyn RenderPipe>,
}

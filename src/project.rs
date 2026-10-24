use std::{path::PathBuf, sync::Arc};

use rayon::ThreadPool;

use crate::render_pipe::{CPURenderPipe, RenderPipe};

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    project_path: PathBuf,
    pub renderer: Box<dyn RenderPipe>,
    thread_pool: Arc<ThreadPool>,
}

impl ProjectRuntime {
    pub fn new(project_path: PathBuf, thread_pool: Arc<ThreadPool>) -> Self {
        let renderer: Box<dyn RenderPipe> = Box::new(CPURenderPipe::new(1920, 1080));
        ProjectRuntime {
            project_path: project_path,
            renderer,
            thread_pool,
        }
    }
}

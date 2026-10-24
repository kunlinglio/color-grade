use std::{path::PathBuf, sync::Arc};

use rayon::ThreadPool;

use crate::{
    media::MediaManager,
    render_pipe::{CPURenderPipe, RenderPipe},
};

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    project_path: PathBuf,
    pub renderer: Box<dyn RenderPipe>,
    thread_pool: Arc<ThreadPool>,
    pub media_manager: MediaManager,
}

impl ProjectRuntime {
    pub fn new(project_path: PathBuf, thread_pool: Arc<ThreadPool>) -> Self {
        let renderer: Box<dyn RenderPipe> =
            Box::new(CPURenderPipe::new(1920, 1080, thread_pool.clone()));
        let media_manager = MediaManager::new(project_path.clone(), thread_pool.clone());
        ProjectRuntime {
            project_path: project_path,
            renderer,
            thread_pool,
            media_manager,
        }
    }
}

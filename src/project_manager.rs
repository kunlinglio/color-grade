use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rayon::ThreadPool;

use crate::project::comm::ProjectRuntimeHandler;
use crate::project::runtime::{ProjectRuntime, ProjectRuntimeThread};

pub struct ProjectManager {
    /// Project Name -> Project Runtime Handler
    opened: HashMap<String, ProjectRuntimeThread>,
}

impl ProjectManager {
    pub fn new() -> Self {
        ProjectManager {
            opened: HashMap::new(),
        }
    }
    pub fn open_project(
        &mut self,
        path: PathBuf,
        thread_pool: Arc<ThreadPool>,
    ) -> ProjectRuntimeHandler {
        // TODO: Handle open same path multiple times
        let runtime = ProjectRuntime::new(path.clone(), thread_pool);
        let (thread, handler) = runtime.run();
        self.opened
            .insert(path.to_string_lossy().into_owned(), thread); // FIXME: Use real project name rather than the path
        handler
    }
    // TODO: Support close a project
}

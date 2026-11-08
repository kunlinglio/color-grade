use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::ThreadPool;

use crate::project::ProjectRuntimeHandler;
use crate::project::{ProjectRuntime, ProjectRuntimeThread};

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
    /// If you open a project twice, this will return Err(_)
    pub fn open_project(
        &mut self,
        path: PathBuf,
        thread_pool: Arc<ThreadPool>,
    ) -> Result<ProjectRuntimeHandler, ()> {
        if self
            .opened
            .contains_key(&path.to_string_lossy().into_owned())
        {
            return Err(());
        }
        let runtime = ProjectRuntime::new(path.clone(), thread_pool);
        let (thread, handler) = runtime.run();
        self.opened
            .insert(path.to_string_lossy().into_owned(), thread); // FIXME: Use real project name rather than the path
        Ok(handler)
    }
    pub fn close_project(&mut self, path: &Path) {
        let rt = self
            .opened
            .remove(&path.to_string_lossy().into_owned())
            .unwrap();
        rt.quit();
    }
}

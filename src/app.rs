use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::*;
use rayon::ThreadPool;

use crate::project_manager::ProjectManager;
use crate::ui;

pub struct ColorGradeApp {
    thread_pool: Arc<ThreadPool>,
    project_manager: ProjectManager,
}

impl ColorGradeApp {
    pub fn new() -> Self {
        let thread_pool_builder = rayon::ThreadPoolBuilder::new()
            .num_threads(std::thread::available_parallelism().unwrap().get())
            .thread_name(|index| format!("Rayon worker for CG #{index}"));
        ColorGradeApp {
            thread_pool: Arc::new(thread_pool_builder.build().unwrap()),
            project_manager: ProjectManager::new(),
        }
    }

    pub fn open_project(&mut self, path: PathBuf, app: &mut App) {
        let project_handler = self
            .project_manager
            .open_project(path.clone(), self.thread_pool.clone());
        ui::MainWindow::open(app, project_handler, path);
    }

    pub fn close_project(&mut self, path: &Path) {
        self.project_manager.close_project(path);
    }
}

impl Global for ColorGradeApp {}

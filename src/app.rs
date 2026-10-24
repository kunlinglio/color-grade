use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::*;
use rayon::ThreadPool;

use crate::project;
use crate::ui;

pub struct ColorGradeApp {
    thread_pool: Arc<ThreadPool>,
}

impl ColorGradeApp {
    pub fn new() -> Self {
        let thread_poop_builder = rayon::ThreadPoolBuilder::new()
            .num_threads(std::thread::available_parallelism().unwrap().get())
            .thread_name(|index| format!("Rayon worker #{index}"));
        ColorGradeApp {
            thread_pool: Arc::new(thread_poop_builder.build().unwrap()),
        }
    }

    pub fn open_project(&self, path: PathBuf, app: &mut App) {
        let runtime = project::ProjectRuntime::new(path, self.thread_pool.clone());
        ui::MainWindow::open(app, runtime);
    }
}

impl Global for ColorGradeApp {}

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::*;

use crate::project;
use crate::ui;

pub struct ColorGradeApp;

impl ColorGradeApp {
    pub fn new() -> Self {
        ColorGradeApp {}
    }

    pub fn open_project(&self, path: PathBuf, app: &mut App) {
        let runtime = Arc::new(project::ProjectRuntime::new(path));
        ui::MainWindow::open(app, runtime);
    }
}

impl Global for ColorGradeApp {}

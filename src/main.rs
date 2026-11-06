#[macro_use]
mod logger;
mod app;
mod media;
mod project;
mod project_manager;
mod render;
mod thumbnail;
mod ui;

use crate::app::ColorGradeApp;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    logger::init(logger::LogLevel::Debug);
    let cg_app = ColorGradeApp::new();
    cg_app.run();
}

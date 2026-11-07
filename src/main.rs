use color_grade::app::ColorGradeApp;
use color_grade::logger;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    logger::init(logger::LogLevel::Debug);
    let cg_app = ColorGradeApp::new();
    cg_app.run();
}

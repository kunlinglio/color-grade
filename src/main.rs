mod app;
mod ui;

fn main() {
    let app = app::ColorGradeApp::new();
    app.run();
}

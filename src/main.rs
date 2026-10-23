mod app;
mod project;
mod render_pipe;
mod ui;

fn main() {
    let app = app::ColorGradeApp::new();
    app.run();
}

use crate::app::ColorGradeApp;

mod app;
#[macro_use]
mod logger;
mod media;
mod project;
mod render_pipe;
mod ui;

fn main() {
    logger::init(logger::LogLevel::Debug);
    let cg_app = ColorGradeApp::new();
    gpui_kit::application().run(move |app| {
        // This must be called before using any GPUI Component features.
        gpui_kit::init(app);

        app.set_global(cg_app);

        // Quit the app once the last window has been closed.
        app.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        ui::ProjectManagerWindow::open(app).expect("Failed to open Project Manager Window");

        // Get focus on debug mod
        // TODO: Make this configuratble
        #[cfg(debug_assertions)]
        {
            app.activate(true);
        }
    });
}

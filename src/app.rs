use gpui_kit::*;

use crate::ui;

pub struct ColorGradeApp;

impl ColorGradeApp {
    pub fn new() -> Self {
        ColorGradeApp {}
    }

    pub fn run(&self) {
        gpui_kit::application().run(move |app| {
            // This must be called before using any GPUI Component features.
            gpui_kit::init(app);

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
}

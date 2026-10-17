mod app;

use gpui_kit::*;

fn main() {
    gpui_kit::application().run(move |cx| {
        // This must be called before using any GPUI Component features.
        gpui_kit::init(cx);

        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| app::ColorGrade::new())
        })
        .expect("Failed to open window");
    });
}
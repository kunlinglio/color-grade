use std::sync::Arc;

use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;

const IMG_PATH: &str = "./test_photo.jpg";

pub struct ColorGrade {
    image: Option<Arc<RenderImage>>,
}

impl ColorGrade {
    pub fn new() -> Self {
        ColorGrade { image: None }
    }

    pub fn load_img(&mut self) {
        let dyn_img =
            image::open(IMG_PATH).expect(&format!("Failed to open the image at {IMG_PATH}"));
        let rgba = dyn_img.to_rgba8();
        let frame = image::Frame::new(rgba);
        let rendered_frame = RenderImage::new(vec![frame]);
        self.image = Some(Arc::new(rendered_frame));
    }
}

impl Render for ColorGrade {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(
                Button::new("ok")
                    .primary()
                    .label("Let's Go!")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.load_img();
                        cx.notify();
                    })),
            )
            .child(match &self.image {
                Some(i) => img(i.clone()).into_any_element(),
                None => "Image not loaded".into_any_element(),
            })
    }
}

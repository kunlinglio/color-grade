use std::sync::Arc;

use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;

const IMG_PATH: &str = "./test_photo.jpg";

pub struct Monitor {
    image: Option<Arc<RenderImage>>,
}

impl Monitor {
    pub fn new() -> Self {
        Monitor { image: None }
    }

    fn load_img(&mut self) {
        let dyn_img =
            image::open(IMG_PATH).expect(&format!("Failed to open the image at {IMG_PATH}"));
        let mut rgba = dyn_img.to_rgba8();
        // RenderImage stores pixels in BGRA order; swap R and B to match.
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let frame = image::Frame::new(rgba);
        let rendered_frame = RenderImage::new(vec![frame]);
        self.image = Some(Arc::new(rendered_frame));
    }
}

impl Render for Monitor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::div()
            .v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(
                Button::new("load-image")
                    .primary()
                    .label("Load Image")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.load_img();
                        cx.notify();
                    })),
            )
            .child(match &self.image {
                Some(i) => img(i.clone())
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => "Image not loaded".into_any_element(),
            })
    }
}

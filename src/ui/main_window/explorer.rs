use gpui_kit::*;

pub struct Explorer {}

impl Explorer {
    pub fn new() -> Self {
        Explorer {}
    }
}

impl Render for Explorer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        text!("Explorer View")
    }
}

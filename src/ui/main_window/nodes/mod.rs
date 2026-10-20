use gpui_kit::*;

pub struct Nodes {}

impl Nodes {
    pub fn new() -> Self {
        Nodes {}
    }
}

impl Render for Nodes {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        text!("Nodes View")
    }
}

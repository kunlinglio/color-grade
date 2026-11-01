use gpui_kit::*;

use crate::ui::components::Text;

pub struct Nodes {}

impl Nodes {
    pub fn new() -> Self {
        Nodes {}
    }
}

impl Render for Nodes {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(Text::new("Nodes").title())
    }
}

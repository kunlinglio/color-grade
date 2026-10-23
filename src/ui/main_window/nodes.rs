use gpui_kit::component::button::*;
use gpui_kit::*;

use super::event;

pub struct Nodes {}

impl Nodes {
    pub fn new() -> Self {
        Nodes {}
    }
}

impl EventEmitter<event::RenderRequested> for Nodes {}

impl Render for Nodes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Button::new("test")
                .primary()
                .label("Render")
                .on_click(cx.listener(|_this, _, _, cx| {
                    cx.emit(event::RenderRequested);
                })),
        )
    }
}

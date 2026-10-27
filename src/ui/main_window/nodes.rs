use gpui_kit::component::button::*;
use gpui_kit::*;

use crate::project::comm::ProjectRuntimeHandler;

use super::event;

pub struct Nodes {
    runtime: ProjectRuntimeHandler,
}

impl Nodes {
    pub fn new(runtime: ProjectRuntimeHandler) -> Self {
        Nodes { runtime }
    }
}

impl EventEmitter<event::RenderRequested> for Nodes {}

impl Render for Nodes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Button::new("test")
                .primary()
                .label("Render")
                .on_click(cx.listener(|this, _event, _win, _cx| {
                    this.runtime.notify_workflow_changed();
                })),
        )
    }
}

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ui::components::Text;
use crate::ui::get_theme;

use super::NODES_PADDING;

pub(super) struct NodePosHud {
    pos_info: Option<PosInfo>,
}

#[derive(PartialEq)]
pub(super) struct PosInfo {
    pub(super) x: f32,
    pub(super) y: f32,
}

impl NodePosHud {
    pub(super) fn new() -> Self {
        Self { pos_info: None }
    }
    pub(super) fn update(&mut self, cx: &mut Context<Self>, new: PosInfo) {
        if let Some(pos_info) = &mut self.pos_info {
            if *pos_info == new {
                return;
            }
        }
        self.pos_info = Some(new);
        cx.notify();
    }
    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        if self.pos_info == None {
            return;
        }
        self.pos_info = None;
        cx.notify();
    }
}

impl Render for NodePosHud {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let Some(info) = &self.pos_info else {
            return div().into_any_element();
        };

        div()
            .absolute()
            .left(px(NODES_PADDING))
            .bottom(px(NODES_PADDING))
            .px_2()
            .py_1()
            .rounded(theme.radii.small)
            .bg(theme.colors.background.elevated)
            .child(Text::new(format!("x: {:.1}  y: {:.1}", info.x, info.y,)).content())
            .into_any_element()
    }
}

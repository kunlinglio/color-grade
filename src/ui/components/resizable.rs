use std::rc::Rc;

use gpui_kit::base::{ElementExt, ResizablePanel, StyledExt};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::ui::get_theme;

fn handle_appearance() -> base::ResizeHandleRenderer {
    Rc::new(|handle, _window, cx| {
        let theme = get_theme(cx);
        let axis = handle.axis();

        Some(
            div()
                .flex_none()
                .flex()
                .bg(theme.colors.border.subtle)
                .map(|l| match axis {
                    Axis::Horizontal => l.w(theme.border_width).h_full().items_center(),
                    _ => l
                        .h(theme.border_width)
                        .w_full()
                        .items_start()
                        .justify_center(),
                })
                .into_any_element(),
        )
    })
}

pub fn resizable_panel() -> ResizablePanel {
    gpui_kit::base::resizable_panel()
}

pub fn h_resizable(id: impl Into<ElementId>) -> base::ResizablePanelGroup {
    gpui_kit::base::h_resizable(id).with_handle_appearance(handle_appearance())
}

pub fn v_resizable(id: impl Into<ElementId>) -> base::ResizablePanelGroup {
    gpui_kit::base::v_resizable(id).with_handle_appearance(handle_appearance())
}

#[derive(IntoElement)]
pub struct ResizeSensitive {
    on_resize: Option<Box<dyn Fn(Bounds<Pixels>, &mut Window, &mut App)>>,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl ResizeSensitive {
    pub fn new() -> Self {
        Self {
            on_resize: None,
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
    pub fn on_resize(
        mut self,
        cb: impl Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(Box::new(cb));
        self
    }
}

impl Styled for ResizeSensitive {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ResizeSensitive {
    fn render(self, _window: &mut Window, _app: &mut App) -> impl IntoElement {
        div()
            .on_prepaint(move |bounds, window, app| {
                if let Some(on_resize) = self.on_resize {
                    on_resize(bounds, window, app);
                };
            })
            .children(self.children)
            .refine_style(&self.style)
    }
}

impl ParentElement for ResizeSensitive {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

use std::rc::Rc;

use gpui_kit::base::{ElementExt, ResizablePanel, StyledExt};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::ui::get_theme;

/// The handle used for divider.
/// When idle: Show a grew divider line,
/// When hover: Show line with accent color
fn divider_handle_appearance() -> base::ResizeHandleRenderer {
    Rc::new(|handle, _window, cx| {
        let theme = get_theme(cx);
        Some(
            // A wrapper to make displayed line slightly shorter
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .map(|wrapper| match handle.axis() {
                    Axis::Horizontal => wrapper.py_3(),
                    Axis::Vertical => wrapper.px_3(),
                })
                .child(
                    div()
                        .flex_none()
                        .bg(match handle.state() {
                            base::ResizeHandleState::Idle | base::ResizeHandleState::Hovered => {
                                theme.colors.border.subtle
                            }
                            base::ResizeHandleState::Pressed
                            | base::ResizeHandleState::Dragging => theme.colors.accent,
                        })
                        .rounded_full()
                        .map(|line| match handle.axis() {
                            Axis::Horizontal => line.w(theme.divider_width).h_full().py_2(),
                            Axis::Vertical => line.h(theme.divider_width).w_full().px_2(),
                        }),
                )
                .into_any_element(),
        )
    })
}

/// The handle used for floating panel.
/// When idle: Show a grew divider line,
/// When hover: Show line with accent color
fn floating_handle_appearance() -> base::ResizeHandleRenderer {
    Rc::new(|handle, _window, cx| {
        let theme = get_theme(cx);
        Some(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex_none()
                        .bg(match handle.state() {
                            base::ResizeHandleState::Idle | base::ResizeHandleState::Hovered => {
                                transparent_black()
                            }
                            base::ResizeHandleState::Pressed
                            | base::ResizeHandleState::Dragging => theme.colors.accent.into(),
                        })
                        .rounded_full()
                        .map(|line| match handle.axis() {
                            Axis::Horizontal => line.w(theme.divider_width).h_full(),
                            Axis::Vertical => line.h(theme.divider_width).w_full(),
                        }),
                )
                .into_any_element(),
        )
    })
}

pub fn resizable_panel() -> ResizablePanel {
    gpui_kit::base::resizable_panel()
}

pub fn v_resizable(id: impl Into<ElementId>) -> base::ResizablePanelGroup {
    gpui_kit::base::v_resizable(id).with_handle_appearance(divider_handle_appearance())
}

pub fn h_floating_resizable(id: impl Into<ElementId>) -> base::ResizablePanelGroup {
    gpui_kit::base::h_resizable(id).with_handle_appearance(floating_handle_appearance())
}

pub fn v_floating_resizable(id: impl Into<ElementId>) -> base::ResizablePanelGroup {
    gpui_kit::base::v_resizable(id).with_handle_appearance(floating_handle_appearance())
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

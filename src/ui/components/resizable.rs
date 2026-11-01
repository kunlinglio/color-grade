use std::rc::Rc;

use gpui_kit::base::ResizablePanel;
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
                .bg(theme.panel_border_color)
                .map(|l| match axis {
                    Axis::Horizontal => l.w(theme.panel_border_width).h_full().items_center(),
                    _ => l
                        .h(theme.panel_border_width)
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

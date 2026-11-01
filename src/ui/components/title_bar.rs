use gpui_kit::base::InteractiveElementExt as _;
use gpui_kit::base::StyledExt;
use gpui_kit::*;

use crate::ui::get_theme;

const TITLE_BAR_HEIGHT: Pixels = px(32.);
const MACOS_LEFT_PADDING: Pixels = px(80.);
const OTHER_LEFT_PADDING: Pixels = px(12.);

#[derive(IntoElement)]
pub struct TitleBar {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl TitleBar {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Styled for TitleBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for TitleBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

struct DragState {
    should_move: bool,
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = get_theme(cx);
        let is_macos = cfg!(target_os = "macos");
        let state = window.use_state(cx, |_, _| DragState { should_move: false });

        div()
            .id("title-bar")
            .flex()
            .items_center()
            .justify_between()
            .h(TITLE_BAR_HEIGHT)
            .pl(if is_macos {
                MACOS_LEFT_PADDING
            } else {
                OTHER_LEFT_PADDING
            })
            .pr(px(8.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.panel_border_color)
            .on_double_click(move |_event, window, _cx| {
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else {
                    window.zoom_window();
                }
            })
            .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                state.should_move = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = true;
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }),
            )
            .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                if state.should_move {
                    state.should_move = false;
                    window.start_window_move();
                }
            }))
            .refine_style(&self.style)
            .children(self.children)
    }
}

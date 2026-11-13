use gpui_kit::assets::IconName;
use gpui_kit::base::InteractiveElementExt as _;
use gpui_kit::base::StyledExt;
use gpui_kit::*;

use crate::ui::components::Icon;
use crate::ui::get_theme;

const TITLE_BAR_HEIGHT: Pixels = px(32.);
const MACOS_LEFT_PADDING: Pixels = px(80.);
const OTHER_LEFT_PADDING: Pixels = px(12.);
/// Width of a single minimize / maximize / close button on Windows and Linux.
/// Matches the native Windows caption-button width.
const WINDOW_CONTROL_WIDTH: Pixels = px(46.);

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

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = get_theme(cx);
        let title_bar = div()
            .id("title-bar")
            .flex()
            .items_center()
            .h(TITLE_BAR_HEIGHT)
            .pl(if cfg!(target_os = "macos") {
                MACOS_LEFT_PADDING
            } else {
                OTHER_LEFT_PADDING
            })
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.colors.border.subtle)
            .refine_style(&self.style)
            .child(
                div()
                    .id("bar")
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag)
                    .children(self.children),
            )
            .child(WindowControls);

        if cfg!(target_os = "macos") {
            // macOS keeps its traffic lights on the left and reserves a little
            // right padding for the last child
            title_bar
                .pr(px(8.))
                .on_double_click(move |_, window, _| window.titlebar_double_click())
        } else if cfg!(target_os = "linux") {
            title_bar.on_double_click(move |_, window, _| window.zoom_window())
        } else {
            title_bar
        }
    }
}

/// The minimize / maximize / close buttons drawn on the right of the title bar.
///
/// On macOS the system draws the traffic lights on the left, so this renders
/// nothing there.
#[derive(IntoElement)]
struct WindowControls;

impl RenderOnce for WindowControls {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if cfg!(target_os = "macos") {
            return div().id("window-controls");
        }

        div()
            .id("window-controls")
            .flex()
            .items_center()
            .h_full()
            .child(ControlIcon::minimize())
            .child(if window.is_maximized() {
                ControlIcon::restore()
            } else {
                ControlIcon::maximize()
            })
            .child(ControlIcon::close())
    }
}

/// A single window control button.
#[derive(IntoElement)]
enum ControlIcon {
    Minimize,
    Restore,
    Maximize,
    Close,
}

impl ControlIcon {
    fn minimize() -> Self {
        Self::Minimize
    }

    fn restore() -> Self {
        Self::Restore
    }

    fn maximize() -> Self {
        Self::Maximize
    }

    fn close() -> Self {
        Self::Close
    }

    fn id(&self) -> &'static str {
        match self {
            Self::Minimize => "minimize",
            Self::Restore => "restore",
            Self::Maximize => "maximize",
            Self::Close => "close",
        }
    }

    fn icon(&self) -> IconName {
        match self {
            Self::Minimize => IconName::WindowMinimize,
            Self::Restore => IconName::WindowRestore,
            Self::Maximize => IconName::WindowMaximize,
            Self::Close => IconName::WindowClose,
        }
    }

    fn window_control_area(&self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Restore | Self::Maximize => WindowControlArea::Max,
            Self::Close => WindowControlArea::Close,
        }
    }

    fn is_close(&self) -> bool {
        matches!(self, Self::Close)
    }
}

impl RenderOnce for ControlIcon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = get_theme(cx);
        let is_close = self.is_close();
        let hover_bg = if is_close {
            rgb(0xe81123)
        } else {
            theme.colors.background.surface
        };
        let active_bg = if is_close {
            rgb(0xc42b1c)
        } else {
            theme.colors.background.elevated
        };
        let hover_fg = if is_close {
            theme.colors.text.inverse
        } else {
            theme.colors.text.primary
        };

        div()
            .id(self.id())
            .flex()
            .w(WINDOW_CONTROL_WIDTH)
            .h_full()
            .items_center()
            .justify_center()
            .text_color(theme.colors.text.primary)
            .window_control_area(self.window_control_area())
            .hover(move |style| style.bg(hover_bg).text_color(hover_fg))
            .active(move |style| style.bg(active_bg).text_color(hover_fg))
            .child(Icon::new(self.icon()))
    }
}

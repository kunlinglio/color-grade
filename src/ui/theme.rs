use gpui_kit::*;

#[derive(Clone, Copy)]
pub struct Theme {
    // Background colors
    pub window_background_color: Rgba,
    pub panel_background_color: Rgba,
    // Button colors
    pub hover_button_color: Rgba,
    pub selected_button_color: Rgba,
    // Font styles
    pub title_font_color: Rgba,
    pub content_font_color: Rgba,
    pub title_font_size: Pixels,
    pub content_font_size: Pixels,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            window_background_color: rgb(0x101010),
            panel_background_color: rgb(0x202020),
            hover_button_color: rgb(0x505050),
            selected_button_color: rgba(0x3b82f633),
            title_font_color: rgb(0xffffff),
            content_font_color: rgb(0xeeeeee),
            title_font_size: px(20.),
            content_font_size: px(18.),
        }
    }
}

impl Global for Theme {}

pub fn get_theme(cx: &App) -> Theme {
    *Theme::global(cx)
}

pub fn _set_theme(new_theme: Theme, cx: &mut App) {
    Theme::update_global(cx, |slot, _app| *slot = new_theme);
}

/// Register the default theme and make every window re-render when it changes.
pub fn init_theme(cx: &mut App) {
    cx.set_global(Theme::default());
    cx.observe_global::<Theme>(|cx| cx.refresh_windows())
        .detach();
}

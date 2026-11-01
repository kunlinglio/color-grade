use gpui_kit::*;

#[inline]
pub fn get_theme(app: &App) -> Theme {
    *Theme::global(app)
}

pub fn set_theme(new_theme: Theme, app: &mut App) {
    Theme::update_global(app, |slot, _app| *slot = new_theme);
}

/// Register the default theme and make every window re-render when it changes.
pub fn init_theme(app: &mut App) {
    app.set_global(Theme::default());
    app.observe_global::<Theme>(|cx| cx.refresh_windows())
        .detach();
}

#[derive(Clone, Copy)]
pub struct Theme {
    // Background colors
    pub window_background_color: Rgba,

    // Panel styles
    pub panel_border_color: Rgba,
    pub panel_border_width: Pixels,

    // Button colors
    pub border_radius: Pixels,
    /// Theme for primary button, e.g. confirm
    pub primary_button: ButtonTheme,
    /// Theme for normal button, e.g. cancel
    pub normal_button: ButtonTheme,

    // Font styles
    pub title_font: FontTheme,
    pub content_font: FontTheme,

    // Global colors
    pub strike_color: Rgba,

    /// Whether this is the dark theme (used by the title bar toggle).
    pub is_dark: bool,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            // Background colors
            window_background_color: rgb(0x151515),
            // Panel styles
            panel_border_color: rgb(0x222222),
            panel_border_width: px(1.5),
            // Button colors
            border_radius: px(6.),
            primary_button: ButtonTheme::primary(),
            normal_button: ButtonTheme::normal(),
            // Font styles
            title_font: FontTheme {
                color: rgb(0xffffff),
                size: px(20.),
            },
            content_font: FontTheme {
                color: rgb(0xeeeeee),
                size: px(16.),
            },

            // Global colors
            strike_color: rgb(0x224589),
            is_dark: true,
        }
    }

    pub fn light() -> Self {
        Self {
            // Background colors
            window_background_color: rgb(0xeeeeee),
            // Panel styles
            panel_border_color: rgb(0xcccccc),
            panel_border_width: px(1.5),
            // Button colors
            border_radius: px(6.),
            primary_button: ButtonTheme::primary(),
            normal_button: ButtonTheme::normal(),
            // Font styles
            title_font: FontTheme {
                color: rgb(0x111111),
                size: px(20.),
            },
            content_font: FontTheme {
                color: rgb(0x333333),
                size: px(16.),
            },

            // Global colors
            strike_color: rgb(0x224589),
            is_dark: false,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Global for Theme {}

#[derive(Clone, Copy)]
pub struct ButtonTheme {
    pub normal: ColorTheme,
    pub hover: ColorTheme,
    pub click: ColorTheme,
}

impl ButtonTheme {
    fn primary() -> Self {
        Self {
            normal: ColorTheme {
                fg: rgb(0x000000),
                bg: rgb(0xffffff),
            },
            hover: ColorTheme {
                fg: rgb(0x000000),
                bg: rgb(0xeeeeee),
            },
            click: ColorTheme {
                fg: rgb(0x000000),
                bg: rgb(0xcccccc),
            },
        }
    }
    fn normal() -> Self {
        Self {
            normal: ColorTheme {
                fg: rgb(0xffffff),
                bg: rgb(0x000000),
            },
            hover: ColorTheme {
                fg: rgb(0xfffffff),
                bg: rgb(0x101010),
            },
            click: ColorTheme {
                fg: rgb(0xffffff),
                bg: rgb(0x202020),
            },
        }
    }
}

#[derive(Clone, Copy)]
pub struct ColorTheme {
    pub fg: Rgba,
    pub bg: Rgba,
}

#[derive(Clone, Copy)]
pub struct FontTheme {
    pub color: Rgba,
    pub size: Pixels,
}

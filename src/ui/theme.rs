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

/// Application-wide design tokens.
///
/// Components own their visual recipes and choose which token to use for each
/// variant and interaction state.
#[derive(Clone, Copy)]
pub struct Theme {
    pub colors: ThemeColors,
    pub typography: Typography,
    pub radii: Radii,
    pub border_width: Pixels,

    /// Whether this is the dark theme
    pub is_dark: bool,
}

#[derive(Clone, Copy)]
pub struct ThemeColors {
    pub background: BackgroundColors,
    pub text: TextColors,
    pub border: BorderColors,
    pub accent: AccentColors,
    pub focus: Rgba,
}

/// Neutral background hierarchy, from the base canvas to stronger surfaces.
#[derive(Clone, Copy)]
pub struct BackgroundColors {
    pub canvas: Rgba,
    pub surface: Rgba,
    pub elevated: Rgba,
}

/// Text hierarchy. `inverse` is for text rendered on an accent background.
#[derive(Clone, Copy)]
pub struct TextColors {
    pub primary: Rgba,
    pub secondary: Rgba,
    pub muted: Rgba,
    pub inverse: Rgba,
}

#[derive(Clone, Copy)]
pub struct BorderColors {
    pub subtle: Rgba,
}

#[derive(Clone, Copy)]
pub struct Radii {
    pub small: Pixels,
    pub medium: Pixels,
    pub large: Pixels,
}

#[derive(Clone, Copy)]
pub struct AccentColors {
    pub normal: Rgba,
    pub hover: Rgba,
    pub active: Rgba,
}

#[derive(Clone, Copy)]
pub struct Typography {
    pub title: Pixels,
    pub content: Pixels,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            colors: ThemeColors {
                background: BackgroundColors {
                    canvas: rgb(0x09090b),
                    surface: rgb(0x18181b),
                    elevated: rgb(0x27272a),
                },
                text: TextColors {
                    primary: rgb(0xfafafa),
                    secondary: rgb(0xd4d4d8),
                    muted: rgb(0xa1a1aa),
                    inverse: rgb(0xfafafa),
                },
                border: BorderColors {
                    subtle: rgb(0x2f2f33),
                },
                accent: AccentColors {
                    normal: rgb(0x3154c7),
                    hover: rgb(0x3b63dc),
                    active: rgb(0x2744a8),
                },
                focus: rgb(0x7895ff),
            },
            typography: Typography {
                title: px(18.),
                content: px(14.),
            },
            radii: Radii {
                small: px(4.),
                medium: px(6.),
                large: px(8.),
            },
            border_width: px(1.),
            is_dark: true,
        }
    }

    pub fn light() -> Self {
        Self {
            colors: ThemeColors {
                background: BackgroundColors {
                    canvas: rgb(0xf4f4f5),
                    surface: rgb(0xffffff),
                    elevated: rgb(0xe4e4e7),
                },
                text: TextColors {
                    primary: rgb(0x18181b),
                    secondary: rgb(0x3f3f46),
                    muted: rgb(0x71717a),
                    inverse: rgb(0xffffff),
                },
                border: BorderColors {
                    subtle: rgb(0xd4d4d8),
                },
                accent: AccentColors {
                    normal: rgb(0x3154c7),
                    hover: rgb(0x2849b5),
                    active: rgb(0x203b96),
                },
                focus: rgb(0x3154c7),
            },
            typography: Typography {
                title: px(18.),
                content: px(14.),
            },
            radii: Radii {
                small: px(4.),
                medium: px(6.),
                large: px(8.),
            },
            border_width: px(1.),
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

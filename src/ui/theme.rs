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
                    canvas: rgb(0x151515),
                    surface: rgb(0x202020),
                    elevated: rgb(0x2c2c2c),
                },
                text: TextColors {
                    primary: rgb(0xffffff),
                    secondary: rgb(0xeeeeee),
                    muted: rgb(0x999999),
                    inverse: rgb(0xffffff),
                },
                border: BorderColors {
                    subtle: rgb(0x2c2c2c),
                },
                accent: AccentColors {
                    normal: rgb(0x224589),
                    hover: rgb(0x2d56a0),
                    active: rgb(0x19366d),
                },
                focus: rgb(0x4f7dd8),
            },
            typography: Typography {
                title: px(20.),
                content: px(16.),
            },
            radii: Radii {
                small: px(4.),
                medium: px(6.),
                large: px(8.),
            },
            border_width: px(1.5),
            is_dark: true,
        }
    }

    pub fn light() -> Self {
        Self {
            colors: ThemeColors {
                background: BackgroundColors {
                    canvas: rgb(0xeeeeee),
                    surface: rgb(0xffffff),
                    elevated: rgb(0xdddddd),
                },
                text: TextColors {
                    primary: rgb(0x111111),
                    secondary: rgb(0x333333),
                    muted: rgb(0x777777),
                    inverse: rgb(0xffffff),
                },
                border: BorderColors {
                    subtle: rgb(0xcccccc),
                },
                accent: AccentColors {
                    normal: rgb(0x224589),
                    hover: rgb(0x2d56a0),
                    active: rgb(0x19366d),
                },
                focus: rgb(0x224589),
            },
            typography: Typography {
                title: px(20.),
                content: px(16.),
            },
            radii: Radii {
                small: px(4.),
                medium: px(6.),
                large: px(8.),
            },
            border_width: px(1.5),
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

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
    pub colors: ColorTheme,
    pub typography: Typography,
    pub radii: Radii,
    pub divider_width: Pixels,
}

#[derive(Clone, Copy)]
pub struct ColorTheme {
    pub background: BackgroundColors,
    pub text: TextColors,
    pub border: BorderColors,
    pub interactive: InteractiveColors,
    pub accent: Rgba,
    /// Whether this is the dark theme
    pub is_dark: bool,
}

#[derive(Clone, Copy)]
pub struct BackgroundColors {
    pub canvas: Rgba,
    pub surface: Rgba,
    pub elevated: Rgba,
    /// Background of elements that invert against the surface, e.g. primary buttons.
    pub inverse: Rgba,
    /// Persistent fill of the selected control.
    pub selected: Rgba,
}

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
    pub panel: Pixels,
}

/// Neutral interaction fills shared by controls such as buttons and segmented
/// selectors. They form a ramp above whatever surface a control sits on.
#[derive(Clone, Copy)]
pub struct InteractiveColors {
    /// Hover fill for an unselected control.
    pub hover: Rgba,
    pub inverse_hover: Rgba,
    /// Pressed fill for an unselected control.
    pub active: Rgba,
    pub inverse_active: Rgba,
    /// Hover or pressed fill of the selected control.
    pub selected_hover_and_active: Rgba,
}

impl ColorTheme {
    pub fn dark() -> Self {
        Self {
            background: BackgroundColors {
                canvas: rgb(0x090909),
                surface: rgb(0x1a1a1a),
                elevated: rgb(0x2a2a2a),
                selected: rgb(0x343434),
                inverse: rgb(0xffffff),
            },
            text: TextColors {
                primary: rgb(0xfafafa),
                secondary: rgb(0xd4d4d8),
                muted: rgb(0xa1a1aa),
                inverse: rgb(0x000000),
            },
            border: BorderColors {
                subtle: rgb(0x2f2f33),
            },
            interactive: InteractiveColors {
                hover: rgb(0x323232),
                active: rgb(0x3a3a3a),
                selected_hover_and_active: rgb(0x3f3f3f),
                inverse_hover: rgb(0xe6e6e6),
                inverse_active: rgb(0xd4d4d4),
            },
            accent: rgb(0x7895ff),
            is_dark: true,
        }
    }
    pub fn light() -> Self {
        ColorTheme {
            background: BackgroundColors {
                canvas: rgb(0xf4f4f4),
                surface: rgb(0xffffff),
                elevated: rgb(0xe4e4e4),
                inverse: rgb(0x000000),
                selected: rgb(0xdedede),
            },
            text: TextColors {
                primary: rgb(0x181818),
                secondary: rgb(0x3f3f3f),
                muted: rgb(0x717171),
                inverse: rgb(0xffffff),
            },
            border: BorderColors {
                subtle: rgb(0xd4d4d4),
            },
            interactive: InteractiveColors {
                hover: rgb(0xececec),
                inverse_hover: rgb(0x1a1a1a),
                active: rgb(0xe4e4e4),
                inverse_active: rgb(0x333333),
                selected_hover_and_active: rgb(0xd2d2d2),
            },
            accent: rgb(0x3154c7),
            is_dark: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Typography {
    pub title: Pixels,
    pub content: Pixels,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            colors: ColorTheme::dark(),
            typography: Typography {
                title: px(18.),
                content: px(14.),
            },
            radii: Radii {
                small: px(4.),
                medium: px(6.),
                large: px(8.),
                panel: px(12.),
            },
            divider_width: px(2.),
        }
    }
}

impl Global for Theme {}

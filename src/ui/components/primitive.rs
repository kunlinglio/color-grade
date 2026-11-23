use gpui_kit::IntoElement;
use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt;
use gpui_kit::base::input::{Input, InputEditorStyle, InputState};
use gpui_kit::component::spinner::Spinner as BaseSpinner;
use gpui_kit::component::{Sizable as _, Size as ComponentSize};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::ui::get_theme;

#[derive(IntoElement)]
pub struct Text {
    content: SharedString,
    style: StyleRefinement,
    preset: Option<TextPreset>,
}

enum TextPreset {
    Content,
    Title,
    Muted,
}

impl Text {
    /// Create a text component without any style
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            style: StyleRefinement::default(),
            preset: None,
        }
    }
    pub fn title(mut self) -> Self {
        self.preset = Some(TextPreset::Title);
        self
    }
    pub fn content(mut self) -> Self {
        self.preset = Some(TextPreset::Content);
        self
    }
    pub fn muted(mut self) -> Self {
        self.preset = Some(TextPreset::Muted);
        self
    }
}

impl Styled for Text {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Text {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        div()
            .child(self.content)
            .map(|div| match self.preset {
                None => div,
                Some(TextPreset::Content) => div
                    .text_color(theme.colors.text.secondary)
                    .text_size(theme.typography.content),
                Some(TextPreset::Title) => div
                    .text_color(theme.colors.text.primary)
                    .text_size(theme.typography.title)
                    .font_bold(),
                Some(TextPreset::Muted) => div
                    .text_color(theme.colors.text.muted)
                    .text_size(theme.typography.content),
            })
            .refine_style(&self.style)
    }
}

#[derive(IntoElement)]
pub struct Icon {
    icon: component::Icon,
    style: StyleRefinement,
}

impl Icon {
    pub fn new(icon: impl Into<component::Icon>) -> Self {
        Self {
            icon: component::Icon::new(icon),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }

    fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.icon = self.icon.text_color(color);
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        self.icon
            .size(theme.typography.content)
            .refine_style(&self.style)
    }
}

/// A flag element with a distinct versual effect.
/// Can be used for debugging, flaggin todos and so on.
#[derive(IntoElement)]
#[allow(dead_code)]
pub struct Warn {
    content: SharedString,
    style: StyleRefinement,
}

#[allow(dead_code)]
impl Warn {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Warn {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Warn {
    fn render(self, _window: &mut Window, _app: &mut App) -> impl IntoElement {
        div()
            .refine_style(&self.style)
            .child(Text::new(self.content))
            .text_color(rgb(0xffff00))
            .text_size(px(32.))
            .font_bold()
            .bg(rgb(0xff0000))
    }
}

#[derive(IntoElement)]
pub struct Loading {
    size: Option<ComponentSize>,
    color: Option<Hsla>,
    style: StyleRefinement,
}

impl Loading {
    pub fn new() -> Self {
        Self {
            size: None,
            color: None,
            style: StyleRefinement::default(),
        }
    }
    pub fn size(mut self, size: impl Into<ComponentSize>) -> Self {
        self.size = Some(size.into());
        self
    }
}

impl Styled for Loading {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }

    fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Loading {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        div()
            .child(
                BaseSpinner::new()
                    .icon(IconName::LoaderCircle)
                    .with_size(
                        self.size
                            .unwrap_or(ComponentSize::Size(theme.typography.content)),
                    )
                    .color(
                        self.color
                            .unwrap_or_else(|| theme.colors.text.secondary.into()),
                    ),
            )
            .refine_style(&self.style)
    }
}

/// Draws a ring just outside its parent, separated by `gap`.
#[derive(IntoElement)]
pub struct Outline {
    color: Option<Rgba>,
    width: Pixels,
    offset: Pixels,
    radius: Option<Pixels>,
    style: StyleRefinement,
}

impl Outline {
    pub fn new(width: Pixels, offset: Pixels) -> Self {
        Self {
            color: None,
            width,
            offset,
            radius: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn color(mut self, color: impl Into<Rgba>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }
}

impl Styled for Outline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Outline {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        let radius = self.radius.unwrap_or(theme.radii.medium);
        let inset = self.width + self.offset;

        div()
            .absolute()
            .left(-inset)
            .top(-inset)
            .right(-inset)
            .bottom(-inset)
            .rounded(radius + inset)
            .border(self.width)
            .border_color(self.color.unwrap_or(theme.colors.accent))
            .refine_style(&self.style)
    }
}

#[derive(IntoElement)]
pub struct TextInput {
    state: Entity<InputState>,
    width: Option<Pixels>,
    align: TextAlign,
    style: StyleRefinement,
}

impl TextInput {
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            width: None,
            align: TextAlign::Left,
            style: StyleRefinement::default(),
        }
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
}

impl Styled for TextInput {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        let focused = self.state.read(app).focus_handle(app).is_focused(window);
        let accent: Hsla = theme.colors.accent.into();
        let align = self.align;

        // Overwrite gpui-kit's default theme
        self.state.update(app, |state, cx| {
            state.set_text_align(align, cx);
            state.set_editor_style(InputEditorStyle {
                selection: accent.opacity(0.35),
                caret: accent,
                ..Default::default()
            });
        });

        let state = self.state;
        div()
            .flex()
            .items_center()
            .w(self.width.unwrap_or(px(64.)))
            .h(px(24.))
            .px_2()
            .rounded(theme.radii.small)
            .bg(theme.colors.background.elevated)
            .border_1()
            .border_color(if focused {
                theme.colors.accent.into()
            } else {
                transparent_black()
            })
            .text_size(theme.typography.content)
            .text_color(theme.colors.text.primary)
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |_, window, cx| {
                    let handle = state.read(cx).focus_handle(cx);
                    handle.focus(window, cx);
                }
            })
            .on_mouse_down_out(|_, window, cx| window.blur(cx))
            .child(Input::new(&state))
            .refine_style(&self.style)
    }
}

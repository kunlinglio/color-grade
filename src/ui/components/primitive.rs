use gpui_kit::IntoElement;
use gpui_kit::assets::IconName;
use gpui_kit::base::StyledExt;
use gpui_kit::base::slider::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::component::spinner::Spinner as BaseSpinner;
use gpui_kit::component::{Sizable as _, Size as ComponentSize};
use gpui_kit::*;

use crate::ui::get_theme;

pub use gpui_kit::base::slider::SliderState;

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
        let div = div().child(self.content);
        let div = match self.preset {
            None => div,
            Some(TextPreset::Content) => div
                .text_color(theme.colors.text.secondary)
                .text_size(theme.typography.content),
            Some(TextPreset::Title) => div
                .text_color(theme.colors.text.primary)
                .text_size(theme.typography.title),
            Some(TextPreset::Muted) => div
                .text_color(theme.colors.text.muted)
                .text_size(theme.typography.content),
        };
        div.refine_style(&self.style)
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

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    preset: Option<ButtonPreset>,
}

enum ButtonPreset {
    Primary,
    Normal,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
            on_click: None,
            preset: None,
        }
    }
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
    /// Use the emphasized action variant.
    pub fn primary(mut self) -> Self {
        self.preset = Some(ButtonPreset::Primary);
        self
    }
    /// Use the neutral action variant.
    pub fn normal(mut self) -> Self {
        self.preset = Some(ButtonPreset::Normal);
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        let mut button = gpui_kit::base::Button::new(self.id)
            .text_size(theme.typography.content)
            .rounded(theme.radii.medium)
            .p_2()
            .focus_visible(|s| s.border_1().border_color(theme.colors.focus))
            // Consume pointer events so they don't bubble to an interactive
            // ancestor such as the title bar.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(self.children);
        if let Some(on_click) = self.on_click {
            button = button.on_click(move |event, window, cx| {
                cx.stop_propagation();
                on_click(event, window, cx);
            });
        };
        let button = match self.preset {
            None => button,
            Some(ButtonPreset::Normal) => button
                .bg(theme.colors.background.canvas)
                .text_color(theme.colors.text.primary)
                .hover(|s| s.bg(theme.colors.background.surface))
                .active(|s| s.bg(theme.colors.background.elevated)),
            Some(ButtonPreset::Primary) => button
                .bg(theme.colors.accent.normal)
                .text_color(theme.colors.text.inverse)
                .hover(|s| s.bg(theme.colors.accent.hover))
                .active(|s| s.bg(theme.colors.accent.active)),
        };
        button.refine_style(&self.style)
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
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
pub struct Slider {
    state: Entity<SliderState>,
    style: StyleRefinement,
}

impl Slider {
    pub fn new(state: &Entity<SliderState>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        let end = self.state.read(app).percentage().end;

        BaseSlider::new(&self.state)
            .flex()
            .items_center()
            .w_full()
            .child(
                SliderTrack::new(&self.state)
                    .flex()
                    .items_center()
                    .h_6()
                    .w_full()
                    .child(
                        SliderIndicator::new(&self.state)
                            .relative()
                            .w_full()
                            .h(px(4.))
                            .bg(theme.colors.background.elevated)
                            .child(
                                div()
                                    .absolute()
                                    .h_full()
                                    .left(relative(0.))
                                    .right(relative(1. - end))
                                    .bg(theme.colors.accent.normal),
                            )
                            .child(
                                SliderThumb::new(&self.state)
                                    .absolute()
                                    .top(-px(4.))
                                    .left(relative(end))
                                    .ml(-px(6.))
                                    .w(px(12.))
                                    .h(px(12.))
                                    .rounded_full()
                                    .bg(theme.colors.text.primary),
                            ),
                    ),
            )
            .refine_style(&self.style)
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
        let size = self
            .size
            .unwrap_or(ComponentSize::Size(theme.typography.content));
        let color = self
            .color
            .unwrap_or_else(|| theme.colors.text.secondary.into());

        div()
            .child(
                BaseSpinner::new()
                    .icon(IconName::LoaderCircle)
                    .with_size(size)
                    .color(color),
            )
            .refine_style(&self.style)
    }
}

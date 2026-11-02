use gpui_kit::IntoElement;
use gpui_kit::base::StyledExt;
use gpui_kit::base::slider::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
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
                .text_color(theme.content_font.color)
                .text_size(theme.content_font.size),
            Some(TextPreset::Title) => div
                .text_color(theme.title_font.color)
                .text_size(theme.title_font.size),
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
            .size(theme.content_font.size)
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
    /// Use primary preset as theme
    pub fn primary(mut self) -> Self {
        self.preset = Some(ButtonPreset::Primary);
        self
    }
    /// Use normal preset as theme
    pub fn normal(mut self) -> Self {
        self.preset = Some(ButtonPreset::Normal);
        self
    }
    // TODO: Support customizable theme
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
            .text_size(theme.content_font.size)
            .rounded(theme.border_radius)
            .p_2()
            .focus_visible(|s| s.border_1().border_color(theme.strike_color))
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
            Some(ButtonPreset::Normal) => {
                let theme = theme.normal_button;
                button
                    .bg(theme.normal.bg)
                    .text_color(theme.normal.fg)
                    .hover(|s| s.bg(theme.hover.bg).text_color(theme.hover.fg))
                    .active(|s| s.bg(theme.click.bg).text_color(theme.click.fg))
            }
            Some(ButtonPreset::Primary) => {
                let theme = theme.primary_button;
                button
                    .bg(theme.normal.bg)
                    .text_color(theme.normal.fg)
                    .hover(|s| s.bg(theme.hover.bg).text_color(theme.hover.fg))
                    .active(|s| s.bg(theme.click.bg).text_color(theme.click.fg))
            }
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
pub struct Warn {
    content: SharedString,
    style: StyleRefinement,
}

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
                            .bg(theme.content_font.color)
                            .child(
                                div()
                                    .absolute()
                                    .h_full()
                                    .left(relative(0.))
                                    .right(relative(1. - end))
                                    .bg(theme.strike_color),
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
                                    .bg(theme.title_font.color),
                            ),
                    ),
            )
            .refine_style(&self.style)
    }
}

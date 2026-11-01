use gpui_kit::IntoElement;
use gpui_kit::base::StyledExt;
use gpui_kit::*;

use crate::ui::get_theme;

#[derive(IntoElement)]
pub struct Text {
    content: SharedString,
    style: StyleRefinement,
}

impl Text {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Text {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Text {
    fn render(self, _window: &mut Window, _app: &mut App) -> impl IntoElement {
        div().child(self.content).refine_style(&self.style)
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
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        self.icon
            .size(theme.content_font_size)
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
            .text_size(theme.content_font_size)
            .rounded(theme.border_radius)
            .p_2()
            .focus_visible(|s| s.border_1().border_color(theme.strike_color))
            .children(self.children);
        if let Some(on_click) = self.on_click {
            button = button.on_click(move |event, window, cx| on_click(event, window, cx));
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

#[derive(IntoElement)]
pub struct Title {
    content: SharedString,
    style: StyleRefinement,
}

impl Title {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            style: StyleRefinement::default(),
        }
    }
}

impl Styled for Title {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Title {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        div()
            .child(Text::new(self.content))
            .text_color(theme.title_font_color)
            .text_size(theme.title_font_size)
            .refine_style(&self.style)
    }
}

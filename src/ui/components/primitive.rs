use std::rc::Rc;

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
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        div()
            .child(self.content)
            // .text_color(theme.content_font_color)
            // .text_size(theme.content_font_size)
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
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = get_theme(app);
        self.icon
            .text_color(theme.content_font_color)
            .text_size(theme.content_font_size)
            .refine_style(&self.style)
    }
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            children: Vec::new(),
            style: StyleRefinement::default(),
            on_click: None,
        }
    }
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
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
            // default style
            .bg(theme.button_color)
            .text_color(theme.button_font_color)
            .text_size(theme.content_font_size)
            .rounded(theme.border_radius)
            .p_2()
            // activity
            .hover(|s| s.bg(theme.hover_button_color))
            .active(|s| s.bg(theme.hover_button_color))
            .focus_visible(|s| s.border_1().border_color(theme.selected_button_color))
            .children(self.children)
            .refine_style(&self.style);
        if let Some(on_click) = self.on_click {
            button = button.on_click(move |event, window, cx| on_click(event, window, cx));
        };
        button
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

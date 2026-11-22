use std::rc::Rc;

use gpui_kit::IntoElement;
use gpui_kit::base::StyledExt;
use gpui_kit::base::slider::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::ui::{Theme, get_theme};

pub use gpui_kit::base::slider::SliderState;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    children: Vec<AnyElement>,
    style: StyleRefinement,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    preset: Option<ButtonPreset>,
}

enum ButtonPreset {
    Primary,
    Normal,
    Selected,
}

impl ButtonPreset {
    fn apply(self, button: gpui::base::Button, theme: &Theme) -> gpui::base::Button {
        match self {
            // Unselected controls stay transparent so they blend into whatever
            // surface they sit on, like a toolbar button.
            ButtonPreset::Normal => button
                .text_color(theme.colors.text.secondary)
                .hover(|s| s.bg(theme.colors.interactive.hover))
                .active(|s| s.bg(theme.colors.interactive.active)),
            ButtonPreset::Primary => button
                .bg(theme.colors.background.inverse)
                .text_color(theme.colors.text.inverse)
                .hover(|s| s.bg(theme.colors.interactive.inverse_hover))
                .active(|s| s.bg(theme.colors.interactive.inverse_active)),
            ButtonPreset::Selected => button
                .bg(theme.colors.background.selected)
                .text_color(theme.colors.text.secondary)
                .hover(|s| s.bg(theme.colors.interactive.selected_hover_and_active))
                .active(|s| s.bg(theme.colors.interactive.selected_hover_and_active)),
        }
    }
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
        self.on_click = Some(Rc::new(handler));
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
    /// Use the selected variant.
    pub fn selected(mut self) -> Self {
        self.preset = Some(ButtonPreset::Selected);
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
        let on_click = self.on_click.clone();
        gpui_kit::base::Button::new(self.id)
            .text_size(theme.typography.content)
            .rounded(theme.radii.medium)
            .p_2()
            .focus_visible(|s| s.border_1().border_color(theme.colors.accent))
            // Consume pointer events so they don't bubble to an interactive
            // ancestor such as the title bar.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(self.children)
            .on_click(move |event, window, cx| {
                if let Some(on_click) = on_click.as_ref() {
                    cx.stop_propagation();
                    on_click(event, window, cx);
                };
            })
            .map(|button| match self.preset {
                None => button,
                Some(preset) => preset.apply(button, &theme),
            })
            .refine_style(&self.style)
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
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
                                    .bg(theme.colors.accent),
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

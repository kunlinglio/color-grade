use std::rc::Rc;

use strum::IntoEnumIterator;

use gpui_kit::base::StyledExt;
use gpui_kit::*;

use crate::ui::components::{Button, Text};

/// A value that can be an option in a [`SingleSelection`].
pub trait SelectOption: Copy + PartialEq + IntoEnumIterator + 'static {
    /// The stable id of this option.
    fn id(self) -> &'static str;
    /// The display label of this option.
    fn label(self) -> &'static str;
}

/// A horizontal row of buttons where exactly one option is selected.
#[derive(IntoElement)]
pub struct SingleSelect<T: SelectOption> {
    selected: Option<T>,
    on_select: Option<Rc<dyn Fn(T, &mut Window, &mut App)>>,
    style: StyleRefinement,
}

impl<T: SelectOption> SingleSelect<T> {
    pub fn new() -> Self {
        Self {
            selected: None,
            on_select: None,
            style: StyleRefinement::default(),
        }
    }

    /// Mark the currently selected value.
    pub fn select(mut self, value: T) -> Self {
        self.selected = Some(value);
        self
    }

    /// Called with the newly selected value when an option is clicked.
    pub fn on_select(mut self, cb: impl Fn(T, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(cb));
        self
    }
}

impl<T: SelectOption> Styled for SingleSelect<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: SelectOption> RenderOnce for SingleSelect<T> {
    fn render(self, _window: &mut Window, _app: &mut App) -> impl IntoElement {
        let selected = self.selected;
        let on_select = self.on_select;

        div()
            .flex()
            .items_center()
            .gap_1()
            .children(T::iter().map(|value| {
                let is_selected = selected == Some(value);
                let button = Button::new(value.id()).child(Text::new(value.label()));
                let button = if is_selected {
                    button.primary()
                } else {
                    button.normal()
                };
                if let Some(cb) = &on_select {
                    let cb = cb.clone();
                    button.on_click(move |_event, window, app| cb(value, window, app))
                } else {
                    button
                }
            }))
            .refine_style(&self.style)
    }
}

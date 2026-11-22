use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use strum::IntoEnumIterator;

use gpui_kit::base::StyledExt;
use gpui_kit::*;

use crate::ui::components::Button;

/// A value that can be an option in a [`SingleSelection`].
pub trait SelectOption: Copy + PartialEq + IntoEnumIterator + 'static {
    /// The stable id of this option.
    fn id(self) -> &'static str;
    /// The display content of this option.
    fn display(self) -> impl IntoElement;
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
                let on_select = on_select.clone();
                Button::new(value.id())
                    .child(value.display())
                    .map(|button| {
                        if selected == Some(value) {
                            button.selected()
                        } else {
                            button.normal()
                        }
                    })
                    .on_click(move |_event, window, app| {
                        if let Some(cb) = on_select.as_ref() {
                            cb(value, window, app)
                        }
                    })
            }))
            .refine_style(&self.style)
    }
}

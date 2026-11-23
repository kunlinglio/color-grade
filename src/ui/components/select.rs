use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::{ElementExt, Popover, StyledExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use strum::IntoEnumIterator;

use crate::ui::components::{Button, Icon};
use crate::ui::get_theme;

const MENU_PADDING: Pixels = px(4.);

/// A value that can be an option in a [`SingleSelection`].
pub trait SelectOption: Copy + PartialEq + IntoEnumIterator + 'static {
    /// The stable id of this option.
    fn id(self) -> &'static str;
    /// The display content of this option.
    fn display(self) -> impl IntoElement;
}

/// A horizontal row of buttons where exactly one option is selected.
#[derive(IntoElement)]
pub struct SegmentedSelect<T: SelectOption> {
    selected: Option<T>,
    on_select: Option<Rc<dyn Fn(T, &mut Window, &mut App)>>,
    style: StyleRefinement,
}

impl<T: SelectOption> SegmentedSelect<T> {
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

impl<T: SelectOption> Styled for SegmentedSelect<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: SelectOption> RenderOnce for SegmentedSelect<T> {
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
                    .p_1()
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

/// A macOS-style pop-up button
pub struct PopupButton<T: SelectOption> {
    selected: Option<T>,
    open: bool,
    on_select_changed: Option<Rc<dyn Fn(T, &mut Window, &mut App)>>,
    style: StyleRefinement,
    id: ElementId,
    width: Option<Pixels>,
    height: Option<Pixels>,
    min_width: Option<Pixels>,
}

impl<T: SelectOption> PopupButton<T> {
    pub fn new(id: impl Into<ElementId>, selected: Option<T>) -> Self {
        Self {
            selected,
            open: false,
            on_select_changed: None,
            style: StyleRefinement::default(),
            id: id.into(),
            width: None,
            height: None,
            min_width: None,
        }
    }

    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = Some(width);
        self
    }

    pub fn set_selected(&mut self, value: Option<T>, cx: &mut Context<Self>) {
        if self.selected == value {
            return;
        }
        self.selected = value;
        cx.notify();
    }

    /// Called with the newly selected value when an option is clicked.
    pub fn on_select_changed(mut self, cb: impl Fn(T, &mut Window, &mut App) + 'static) -> Self {
        self.on_select_changed = Some(Rc::new(cb));
        self
    }

    fn select(&mut self, value: T, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
        if self.selected == Some(value) {
            return;
        }
        self.selected = Some(value);
        if let Some(cb) = self.on_select_changed.as_ref() {
            cb(value, window, cx);
        }
    }
}

impl<T: SelectOption> Styled for PopupButton<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: SelectOption> Render for PopupButton<T> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let selected = self.selected;
        let open = self.open;
        let this = cx.entity();
        let this_for_resize = this.clone();
        let this_for_min_width = this.clone();
        let id = self.id.clone();
        let trigger_height = self.height.unwrap_or(px(0.));
        let menu_offset = -(trigger_height + MENU_PADDING);

        Popover::new(("popup-button", cx.entity_id()))
            .anchor(Anchor::TopLeft)
            .offset(menu_offset)
            .open(open)
            .on_open_change(cx.listener(|this, open: &bool, _window, cx| {
                this.open = *open;
                cx.notify();
            }))
            .trigger_with(move |_is_open, _window, cx| {
                let this = this_for_min_width;
                Button::new(id)
                    .bg(theme.colors.background.elevated)
                    .p_1()
                    .flex()
                    .justify_start()
                    .items_center()
                    .normal()
                    .allow_propagation()
                    .map(|button| {
                        if let Some(min_width) = this.read(cx).min_width {
                            button.min_w(min_width)
                        } else {
                            button
                        }
                    })
                    .on_prepaint(move |bounds, _window, app| {
                        this_for_resize.update(app, |this, _cx| {
                            this.width = Some(bounds.size.width);
                            this.height = Some(bounds.size.height);
                        })
                    })
                    .child(
                        Icon::new(IconName::ChevronsUpDown)
                            .flex_none()
                            .text_color(theme.colors.text.muted)
                            .size(theme.typography.content + px(2.)),
                    )
                    .child(
                        div().flex_1().flex().justify_center().items_center().child(
                            selected
                                .map(|value| value.display().into_any_element())
                                .unwrap_or_else(|| div().into_any_element()),
                        ),
                    )
                    .into_any_element()
            })
            .content(move |_popover, _window, cx| {
                let selected = this.read(cx).selected;

                div()
                    .v_flex()
                    .id("popup-button-menu")
                    .p(MENU_PADDING)
                    .w(this.read(cx).width.unwrap_or(px(200.)))
                    .overflow_y_scroll()
                    .bg(theme.colors.background.elevated)
                    .rounded(theme.radii.large)
                    .shadow_lg()
                    .gap_1()
                    .children(T::iter().map(|value| {
                        let this = this.clone();
                        let is_selected = selected == Some(value);

                        div()
                            .id(value.id())
                            .w_full()
                            .flex()
                            .items_center()
                            .px_1()
                            .cursor_pointer()
                            .rounded(theme.radii.medium)
                            .text_color(theme.colors.text.secondary)
                            .hover(|element| {
                                if !is_selected {
                                    element.bg(theme.colors.interactive.hover)
                                } else {
                                    element
                                }
                            })
                            .when(is_selected, |element| {
                                element
                                    .text_color(theme.colors.text.primary)
                                    .bg(theme.colors.accent)
                            })
                            .on_click(move |_event, window, cx| {
                                this.update(cx, |this, cx| this.select(value, window, cx));
                            })
                            // .child(div().w(theme.typography.content).flex_none().when(
                            //     is_selected,
                            //     |element| {
                            //         element.child(
                            //             Icon::new(IconName::Check)
                            //                 .text_color(theme.colors.text.primary)
                            //                 .size(theme.typography.content),
                            //         )
                            //     },
                            // ))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .child(value.display()),
                            )
                    }))
                    .into_any_element()
            })
            .refine_style(&self.style)
    }
}

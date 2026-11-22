use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::base::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::*;

use crate::ui::components::{Slider, Text, TextInput};

pub struct SliderRange {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

pub struct SliderWithField {
    label: SharedString,
    value: f32,
    min: f32,
    max: f32,
    precision: usize,
    slider: Entity<SliderState>,
    input: Entity<InputState>,
    _slider_sub: Subscription,
    _input_sub: Subscription,
}

impl SliderWithField {
    pub fn new(
        label: impl Into<SharedString>,
        value: f32,
        range: SliderRange,
        on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let SliderRange { min, max, step } = range;
        let precision = precision_for_step(step);
        let on_change: Rc<dyn Fn(f32, &mut Window, &mut App)> = Rc::new(on_change);

        let slider = cx.new(|_| {
            SliderState::new()
                .min(min)
                .max(max)
                .step(step)
                .default_value(value)
        });
        let input =
            cx.new(|cx| InputState::new(window, cx).default_value(format_value(value, precision)));

        let _slider_sub = cx.subscribe_in(&slider, window, {
            let input = input.clone();
            let on_change = on_change.clone();
            move |this, _emitter, event, window, cx| {
                let SliderEvent::Change(SliderValue::Single(value)) = event else {
                    return;
                };
                this.value = *value;
                let text = format_value(*value, this.precision);
                input.update(cx, |input, cx| input.set_value(text, window, cx));
                on_change(*value, window, cx);
            }
        });

        let _input_sub = cx.subscribe_in(&input, window, {
            let slider = slider.clone();
            let on_change = on_change.clone();
            move |this, _emitter, event, window, cx| match event {
                InputEvent::Change => {
                    let text = this.input.read(cx).value();
                    let Ok(value) = text.trim().parse::<f32>() else {
                        // Ignore intermediate text such as "", "-" or "1.".
                        return;
                    };
                    let value = value.clamp(this.min, this.max);
                    this.value = value;
                    slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                    on_change(value, window, cx);
                }
                InputEvent::PressEnter { .. } => {
                    window.blur(cx);
                }
                InputEvent::Blur => {
                    let text = format_value(this.value, this.precision);
                    let input = this.input.clone();
                    input.update(cx, |input, cx| input.set_value(text, window, cx));
                }
                _ => {}
            }
        });

        Self {
            label: label.into(),
            value,
            min,
            max,
            precision,
            slider,
            input,
            _slider_sub,
            _input_sub,
        }
    }

    pub fn set_value(&mut self, value: f32, window: &mut Window, cx: &mut Context<Self>) {
        let value = value.clamp(self.min, self.max);
        if (self.value - value).abs() <= f32::EPSILON {
            return;
        }
        self.value = value;
        let text = format_value(value, self.precision);
        self.slider
            .update(cx, |slider, cx| slider.set_value(value, window, cx));
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        cx.notify();
    }
}

impl Render for SliderWithField {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .w_full()
            .gap_2()
            .child(
                div()
                    .flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(Text::new(self.label.clone()).content())
                    .child(
                        TextInput::new(&self.input)
                            .width(px(56.))
                            .align(TextAlign::Right),
                    ),
            )
            .child(Slider::new(&self.slider))
    }
}

fn precision_for_step(step: f32) -> usize {
    if step >= 1. {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    }
}

fn format_value(value: f32, precision: usize) -> String {
    format!("{:.*}", precision, value)
}

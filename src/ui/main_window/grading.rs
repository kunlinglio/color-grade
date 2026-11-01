use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::comm::ProjectRuntimeHandler;
use crate::ui::components::{Slider, Text};

/// Color grading panel
pub struct Grading {
    runtime: ProjectRuntimeHandler,
    brightness: Entity<SliderState>,
    saturation: Entity<SliderState>,
}

impl Grading {
    pub fn new(cx: &mut Context<Self>, runtime: ProjectRuntimeHandler) -> Self {
        let brightness = cx.new(|_cx| {
            SliderState::new()
                .min(0.0)
                .max(2.0)
                .step(0.01)
                .default_value(1.0)
        });
        let saturation = cx.new(|_cx| {
            SliderState::new()
                .min(0.0)
                .max(2.0)
                .step(0.01)
                .default_value(1.0)
        });

        cx.subscribe(&brightness, |this, _state, event, _cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event {
                this.runtime.set_brightness(*value);
            }
        })
        .detach();

        cx.subscribe(&saturation, |this, _state, event, _cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event {
                this.runtime.set_saturation(*value);
            }
        })
        .detach();

        Grading {
            runtime,
            brightness,
            saturation,
        }
    }
}

impl Render for Grading {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(Text::new("Grading").title())
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(Text::new("Brightness").content())
                    .child(Slider::new(&self.brightness)),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(Text::new("Saturation").content())
                    .child(Slider::new(&self.saturation)),
            )
    }
}

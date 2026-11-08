use std::sync::Arc;

use gpui_kit::component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_kit::component::*;
use gpui_kit::*;

use crate::media::media_type::Media;
use crate::project::{ProjectRuntimeHandler, WorkFlowDiff};
use crate::ui::components::{Slider, Text};

/// Color grading panel
pub struct Inspector {
    runtime: ProjectRuntimeHandler,
    brightness: Entity<SliderState>,
    saturation: Entity<SliderState>,
}

impl Inspector {
    pub fn new(
        cx: &mut Context<Self>,
        runtime: ProjectRuntimeHandler,
        selected: Entity<Option<Arc<Media>>>,
    ) -> Self {
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
                this.runtime.update_state(WorkFlowDiff::Brightness(*value));
            }
        })
        .detach();

        cx.subscribe(&saturation, |this, _state, event, _cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event {
                this.runtime.update_state(WorkFlowDiff::Saturation(*value));
            }
        })
        .detach();
        cx.observe(&selected, |this, _selected, cx| {
            let runtime = this.runtime.clone();
            cx.spawn(async move |this, cx| {
                let new = runtime.get_state().await;
                let _ = this.update(cx, |this, cx| {
                    this.brightness.write(
                        cx,
                        SliderState::new()
                            .min(0.0)
                            .max(2.0)
                            .step(0.01)
                            .default_value(new.brightness),
                    );
                    this.saturation.write(
                        cx,
                        SliderState::new()
                            .min(0.0)
                            .max(2.0)
                            .step(0.01)
                            .default_value(new.saturation),
                    );
                });
            })
            .detach();
        })
        .detach();

        Inspector {
            runtime,
            brightness,
            saturation,
        }
    }
}

impl Render for Inspector {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(Text::new("Inspector").title())
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

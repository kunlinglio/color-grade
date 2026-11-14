use gpui_kit::base::slider::{SliderEvent, SliderValue};
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::{NodeKind, NodeKindDiff, WorkFlowDiff, WorkFlowDiffPayload};
use crate::ui::components::{Slider, Text};
use crate::ui::main_window::EditState;

/// Color grading panel
pub struct Inspector {
    edit_state: Entity<EditState>,
    brightness: Entity<SliderState>,
    saturation: Entity<SliderState>,
}

impl Inspector {
    pub fn new(cx: &mut Context<Self>, edit_state: Entity<EditState>) -> Self {
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

        cx.subscribe(&brightness, |this, _state, event, cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event {
                let Some(node_id) = this.edit_state.read(cx).get_selected_node() else {
                    return;
                };
                let diff = WorkFlowDiff {
                    payload: WorkFlowDiffPayload::NodeAttr(
                        node_id,
                        NodeKindDiff::Brightness(*value),
                    ),
                };
                let edit_state = this.edit_state.clone();
                edit_state.update(cx, |this, cx| {
                    let success = this.update(diff, cx);
                    if !success {
                        warn!("Failed to apply brightness changes");
                    };
                });
            }
        })
        .detach();

        cx.subscribe(&saturation, |this, _state, event, cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event {
                let Some(node_id) = this.edit_state.read(cx).get_selected_node() else {
                    return;
                };
                let diff = WorkFlowDiff {
                    payload: WorkFlowDiffPayload::NodeAttr(
                        node_id,
                        NodeKindDiff::Saturation(*value),
                    ),
                };
                let edit_state = this.edit_state.clone();
                edit_state.update(cx, |this, cx| {
                    let success = this.update(diff, cx);
                    if !success {
                        warn!("Failed to apply saturation changes");
                    };
                });
            }
        })
        .detach();

        cx.observe(&edit_state, |this, state, cx| {
            let Some(workflow) = state.read(cx).get_workflow() else {
                // TODO: Handle effect when select nothing
                return;
            };
            let Some(node_id) = state.read(cx).get_selected_node() else {
                return;
            };
            let Some(node) = workflow.get_node(node_id) else {
                warn!(
                    "Cannot find node id {} inside workflow {:?}, the workflow may be changed",
                    node_id, workflow
                );
                return;
            };
            let NodeKind::Normal {
                source: _,
                brightness,
                saturation,
            } = node.attr
            else {
                // TODO: Handle un-inspectable nodes
                return;
            };
            this.brightness.write(
                cx,
                SliderState::new()
                    .min(0.0)
                    .max(2.0)
                    .step(0.01)
                    .default_value(brightness),
            );
            this.saturation.write(
                cx,
                SliderState::new()
                    .min(0.0)
                    .max(2.0)
                    .step(0.01)
                    .default_value(saturation),
            );
            cx.notify()
        })
        .detach();

        Inspector {
            brightness,
            saturation,
            edit_state,
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

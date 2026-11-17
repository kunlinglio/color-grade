use gpui_kit::base::slider::{SliderEvent, SliderValue};
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::{NodeKind, NodeKindDiff, WorkFlowDiff, WorkFlowDiffPayload};
use crate::ui::components::{Slider, Text};
use crate::ui::main_window::{EditState, state};

/// Color grading panel
pub struct Inspector {
    edit_state: Entity<EditState>,
    brightness: Entity<SliderState>,
    _brightness_sub: Subscription,
    saturation: Entity<SliderState>,
    _saturation_sub: Subscription,
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

        let _brightness_sub = cx.subscribe(&brightness, |this, _state, event, cx| {
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
                let id = cx.entity_id();
                if let Err(e) = edit_state.update(cx, |this, cx| this.update(diff, id, cx)) {
                    warn!("Failed to apply brightness changes: {:#}", e);
                    this.sync(cx);
                };
            }
        });

        let _saturation_sub = cx.subscribe(&saturation, |this, _state, event, cx| {
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
                let id = cx.entity_id();
                if let Err(e) = edit_state.update(cx, |this, cx| this.update(diff, id, cx)) {
                    warn!("Failed to apply saturation changes: {:#}", e);
                    this.sync(cx);
                };
            }
        });

        cx.subscribe(
            &edit_state,
            |this, _edit_state, _event: &state::SessionChanged, cx| {
                this.sync(cx);
            },
        )
        .detach();
        cx.subscribe(
            &edit_state,
            |this, _edit_state, _event: &state::SelectedNodeChanged, cx| {
                this.sync(cx);
            },
        )
        .detach();
        cx.subscribe(
            &edit_state,
            |_this, _edit_state, event: &state::NodeAttrChanged, cx| {
                let id = cx.entity_id();
                assert_eq!(
                    id, event.origin,
                    "The selected node attributes should only be changed by inspector"
                )
                // nothing to do, the sliders will update its own  state
            },
        )
        .detach();

        Inspector {
            brightness,
            _brightness_sub,
            saturation,
            _saturation_sub,
            edit_state,
        }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let Some(workflow) = self.edit_state.read(cx).get_workflow() else {
            // TODO: Handle effect when open nothing
            return;
        };
        let Some(node_id) = self.edit_state.read(cx).get_selected_node() else {
            // TODO: Handle effect when select nothing
            return;
        };
        let node = workflow.get_node(node_id).unwrap_or_else(|| {
            panic!(
                "Cannot find node id {} inside workflow {:?}, the workflow may be corrupted",
                node_id, workflow
            )
        });
        let NodeKind::Normal {
            source: _,
            brightness,
            saturation,
        } = node.kind
        else {
            // TODO: Handle un-inspectable nodes
            return;
        };
        Self::set_slider(&self.brightness, brightness, cx);
        Self::set_slider(&self.saturation, saturation, cx);
        cx.notify()
    }

    fn set_slider(slider: &Entity<SliderState>, value: f32, cx: &mut Context<Inspector>) {
        let old_slider = slider.read(cx);
        let current = match old_slider.value() {
            SliderValue::Single(current) => current,
            _ => return,
        };
        if (current - value).abs() <= f32::EPSILON {
            return;
        }
        slider.write(
            cx,
            SliderState::new()
                .min(old_slider.min_value())
                .max(old_slider.max_value())
                .step(old_slider.step_value())
                .default_value(value),
        );
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

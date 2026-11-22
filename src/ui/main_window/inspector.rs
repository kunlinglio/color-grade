use gpui_kit::base::slider::{SliderEvent, SliderValue};
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::{NodeKind, NodeKindDiff, WorkflowDiff, WorkflowDiffPayload};
use crate::ui::components::{Slider, Text, Warn};
use crate::ui::main_window::{EditState, state};

/// Color grading panel
pub struct Inspector {
    edit_state: Entity<EditState>,
    mode: InspectorMode,
}

enum InspectorMode {
    Empty,
    Normal {
        brightness: Entity<SliderState>,
        _brightness_sub: Subscription,
        saturation: Entity<SliderState>,
        _saturation_sub: Subscription,
    },
    Merge {},
    Generator {},
}

impl Inspector {
    pub fn new(cx: &mut Context<Self>, edit_state: Entity<EditState>) -> Self {
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
        let mut inspector = Inspector {
            mode: InspectorMode::Empty,
            edit_state,
        };
        // Sync for the first time
        inspector.sync(cx);
        inspector
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let Some(workflow) = self.edit_state.read(cx).get_workflow() else {
            self.mode = InspectorMode::Empty;
            cx.notify();
            return;
        };
        let Some(node_id) = self.edit_state.read(cx).get_selected_node() else {
            self.mode = InspectorMode::Empty;
            cx.notify();
            return;
        };
        let node = workflow.get_node(node_id).unwrap_or_else(|| {
            panic!(
                "Cannot find node id {} inside workflow {:?}, the workflow may be corrupted",
                node_id, workflow
            )
        });
        self.sync_mode(node.kind.clone(), cx);
        cx.notify();
    }

    fn sync_mode(&mut self, node_kind: NodeKind, cx: &mut Context<Self>) {
        match node_kind {
            NodeKind::Normal {
                brightness: new_brightness,
                saturation: new_saturation,
                ..
            } => {
                if let InspectorMode::Normal {
                    brightness,
                    saturation,
                    ..
                } = &mut self.mode
                {
                    Self::set_slider(brightness, new_brightness, cx);
                    Self::set_slider(saturation, new_saturation, cx);
                } else {
                    let brightness = cx.new(|_cx| {
                        SliderState::new()
                            .min(0.0)
                            .max(2.0)
                            .step(0.01)
                            .default_value(1.0)
                    });
                    Self::set_slider(&brightness, new_brightness, cx);
                    let saturation = cx.new(|_cx| {
                        SliderState::new()
                            .min(0.0)
                            .max(2.0)
                            .step(0.01)
                            .default_value(1.0)
                    });
                    Self::set_slider(&saturation, new_saturation, cx);
                    let _brightness_sub = cx.subscribe(&brightness, |this, _state, event, cx| {
                        if let SliderEvent::Change(SliderValue::Single(value)) = event {
                            let Some(node_id) = this.edit_state.read(cx).get_selected_node() else {
                                return;
                            };
                            let diff = WorkflowDiff {
                                payload: WorkflowDiffPayload::NodeAttr(
                                    node_id,
                                    NodeKindDiff::Brightness(*value),
                                ),
                            };
                            let edit_state = this.edit_state.clone();
                            let id = cx.entity_id();
                            if let Err(e) =
                                edit_state.update(cx, |this, cx| this.update(diff, id, cx))
                            {
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
                            let diff = WorkflowDiff {
                                payload: WorkflowDiffPayload::NodeAttr(
                                    node_id,
                                    NodeKindDiff::Saturation(*value),
                                ),
                            };
                            let edit_state = this.edit_state.clone();
                            let id = cx.entity_id();
                            if let Err(e) =
                                edit_state.update(cx, |this, cx| this.update(diff, id, cx))
                            {
                                warn!("Failed to apply saturation changes: {:#}", e);
                                this.sync(cx);
                            };
                        }
                    });
                    self.mode = InspectorMode::Normal {
                        brightness,
                        _brightness_sub,
                        saturation,
                        _saturation_sub,
                    }
                }
            }
            NodeKind::Merge { sources, kind } => {
                todo!()
            }
            NodeKind::Generator { mode } => {
                todo!()
            }
        }
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
            .child(match &self.mode {
                InspectorMode::Empty => div()
                    .size_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(Text::new("Select a Node to Enable Inspector").content())
                    .into_any_element(),
                InspectorMode::Normal {
                    brightness,
                    saturation,
                    ..
                } => div()
                    .size_full()
                    .gap_2()
                    .v_flex()
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(Text::new("Brightness").content())
                            .child(Slider::new(brightness)),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .child(Text::new("Saturation").content())
                            .child(Slider::new(saturation)),
                    )
                    .into_any_element(),
                InspectorMode::Merge {} => {
                    Warn::new("Merge node inspector is not implemented!").into_any_element()
                }
                InspectorMode::Generator {} => {
                    Warn::new("Generator node inspector is not implemented!").into_any_element()
                }
            })
    }
}

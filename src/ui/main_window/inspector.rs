use gpui_kit::base::slider::{SliderEvent, SliderValue};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::project::{
    MergeNodeKind, NodeAttrDiff, NodeId, NodeKind, WorkflowDiff, WorkflowDiffPayload,
};
use crate::render::BlendMode;
use crate::ui::components::{CheckBox, PopupButton, SelectOption, Slider, Text, Warn};
use crate::ui::main_window::{EditState, state};

impl SelectOption for BlendMode {
    fn id(self) -> &'static str {
        match self {
            BlendMode::Top => "blend-mode-top",
            BlendMode::Bottom => "blend-mode-bottom",
        }
    }
    fn display(self) -> impl IntoElement {
        match self {
            BlendMode::Top => Text::new("Top").content(),
            BlendMode::Bottom => Text::new("Bottom").content(),
        }
    }
}

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
    Merge {
        mode: MergeMode,
    },
    Generator {},
}

enum MergeMode {
    Mixer,
    LayerBlend {
        blend_mode: Entity<PopupButton<BlendMode>>,
    },
}

impl MergeMode {
    fn layer_blend(&self) -> bool {
        match self {
            Self::Mixer => false,
            Self::LayerBlend { .. } => true,
        }
    }
    fn change_layer_blend(
        &mut self,
        enabled: bool,
        node_id: NodeId,
        cx: &mut Context<Inspector>,
    ) -> Option<WorkflowDiff> {
        if self.layer_blend() == enabled {
            return None;
        }
        if enabled {
            *self = Self::from_blend_mode(BlendMode::default(), cx);
            Some(WorkflowDiff {
                payload: WorkflowDiffPayload::NodeAttr(
                    node_id,
                    NodeAttrDiff::MergeMode(MergeNodeKind::LayerBlend(BlendMode::default())),
                ),
            })
        } else {
            *self = Self::Mixer;
            Some(WorkflowDiff {
                payload: WorkflowDiffPayload::NodeAttr(
                    node_id,
                    NodeAttrDiff::MergeMode(MergeNodeKind::Mixer),
                ),
            })
        }
    }
    fn from_blend_mode(blend_mode: BlendMode, cx: &mut Context<Inspector>) -> Self {
        let this = cx.weak_entity();
        let blend_mode = cx.new(|_cx| {
            PopupButton::new("merge-blend-mode", Some(blend_mode))
                .min_width(px(135.))
                .on_select_changed(move |new, _window, app| {
                    let _ = this.update(app, |this, cx| {
                        let Some(node_id) = this.edit_state.read(cx).get_selected_node() else {
                            return;
                        };
                        let diff = WorkflowDiff {
                            payload: WorkflowDiffPayload::NodeAttr(
                                node_id,
                                NodeAttrDiff::MergeMode(MergeNodeKind::LayerBlend(new)),
                            ),
                        };
                        let edit_state = this.edit_state.clone();
                        let id = cx.entity_id();
                        if let Err(e) = edit_state.update(cx, |this, cx| this.update(diff, id, cx))
                        {
                            warn!("Failed to apply blend mode: {:#}", e);
                            this.sync(cx);
                        }
                    });
                })
        });
        MergeMode::LayerBlend { blend_mode }
    }
}

impl InspectorMode {
    fn sync_mode(&mut self, node_kind: NodeKind, cx: &mut Context<Inspector>) {
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
                } = self
                {
                    set_slider(brightness, new_brightness, cx);
                    set_slider(saturation, new_saturation, cx);
                } else {
                    let brightness = cx.new(|_cx| {
                        SliderState::new()
                            .min(0.0)
                            .max(2.0)
                            .step(0.01)
                            .default_value(1.0)
                    });
                    set_slider(&brightness, new_brightness, cx);
                    let saturation = cx.new(|_cx| {
                        SliderState::new()
                            .min(0.0)
                            .max(2.0)
                            .step(0.01)
                            .default_value(1.0)
                    });
                    set_slider(&saturation, new_saturation, cx);
                    let _brightness_sub = cx.subscribe(&brightness, |this, _state, event, cx| {
                        if let SliderEvent::Change(SliderValue::Single(value)) = event {
                            let Some(node_id) = this.edit_state.read(cx).get_selected_node() else {
                                return;
                            };
                            let diff = WorkflowDiff {
                                payload: WorkflowDiffPayload::NodeAttr(
                                    node_id,
                                    NodeAttrDiff::Brightness(*value),
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
                                    NodeAttrDiff::Saturation(*value),
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
                    *self = InspectorMode::Normal {
                        brightness,
                        _brightness_sub,
                        saturation,
                        _saturation_sub,
                    }
                }
            }
            NodeKind::Merge { kind, .. } => {
                *self = match kind {
                    MergeNodeKind::Mixer => InspectorMode::Merge {
                        mode: MergeMode::Mixer,
                    },
                    MergeNodeKind::LayerBlend(blend_mode) => InspectorMode::Merge {
                        mode: MergeMode::from_blend_mode(blend_mode, cx),
                    },
                }
            }
            NodeKind::Generator { .. } => {
                todo!()
            }
        }
    }
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
                );
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
        self.mode.sync_mode(node.kind.clone(), cx);
        cx.notify();
    }
}

impl Render for Inspector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        div()
            .v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(Text::new("Inspector").title())
            .child(
                div() // container for items
                    .size_full()
                    .gap_2()
                    .p_2()
                    .v_flex()
                    .overflow_y_scrollbar()
                    .map(|container| match &self.mode {
                        InspectorMode::Empty => container
                            .justify_center()
                            .items_center()
                            .child(Text::new("Select a Node to Enable Inspector").content())
                            .into_any_element(),
                        InspectorMode::Normal {
                            brightness,
                            saturation,
                            ..
                        } => container
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
                        InspectorMode::Merge { mode } => {
                            let container = container.child(single_line_item(
                                "Use Layer Blender",
                                CheckBox::new("use-layer-blende-mode")
                                    .when(mode.layer_blend(), |check_box| check_box.select())
                                    .on_change(move |enabled, _window, app| {
                                        let _ = this.update(app, |this, cx| {
                                            let InspectorMode::Merge { mode } = &mut this.mode
                                            else {
                                                panic!("Inspector mode unmatched")
                                            };
                                            let Some(node_id) =
                                                this.edit_state.read(cx).get_selected_node()
                                            else {
                                                return;
                                            };
                                            let Some(diff) =
                                                mode.change_layer_blend(enabled, node_id, cx)
                                            else {
                                                return;
                                            };
                                            let edit_state = this.edit_state.clone();
                                            let id = cx.entity_id();
                                            if let Err(e) = edit_state
                                                .update(cx, |this, cx| this.update(diff, id, cx))
                                            {
                                                warn!(
                                                    "Failed to apply saturation changes: {:#}",
                                                    e
                                                );
                                                this.sync(cx);
                                            };
                                            cx.notify();
                                        });
                                    }),
                            ));
                            match mode {
                                // No more options for Mixer, so no more items
                                MergeMode::Mixer => container,
                                MergeMode::LayerBlend { blend_mode } => container
                                    .child(single_line_item("Blende Mode", blend_mode.clone())),
                            }
                            .into_any_element()
                        }
                        InspectorMode::Generator {} => {
                            Warn::new("Generator node inspector is not implemented!")
                                .into_any_element()
                        }
                    }),
            )
    }
}

fn single_line_item(
    name: impl Into<SharedString>,
    component: impl IntoElement,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .justify_between()
        .child(Text::new(name).content())
        .child(component)
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

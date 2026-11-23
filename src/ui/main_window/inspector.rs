use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::project::{
    MergeNodeKind, NodeAttrDiff, NodeId, NodeKind, WorkflowDiff, WorkflowDiffPayload,
};
use crate::render::BlendMode;
use crate::ui::components::{
    CheckBox, PopupButton, SelectOption, SliderRange, SliderWithField, Text, Warn,
};
use crate::ui::main_window::{EditState, state};

impl SelectOption for BlendMode {
    fn id(self) -> &'static str {
        match self {
            BlendMode::Normal => "blend-mode-normal",
            BlendMode::Add => "blend-mode-add",
            BlendMode::Subtract => "blend-mode-subtract",
            BlendMode::Multiply => "blend-mode-multiply",
            BlendMode::Screen => "blend-mode-screen",
            BlendMode::Overlay => "blend-mode-overlay",
            BlendMode::Difference => "blend-mode-difference",
            BlendMode::Darken => "blend-mode-darken",
            BlendMode::Lighten => "blend-mode-lighten",
        }
    }
    fn display(self) -> impl IntoElement {
        match self {
            BlendMode::Normal => Text::new("Normal").content(),
            BlendMode::Add => Text::new("Add").content(),
            BlendMode::Subtract => Text::new("Subtract").content(),
            BlendMode::Multiply => Text::new("Multiply").content(),
            BlendMode::Screen => Text::new("Screen").content(),
            BlendMode::Overlay => Text::new("Overlay").content(),
            BlendMode::Difference => Text::new("Difference").content(),
            BlendMode::Darken => Text::new("Darken").content(),
            BlendMode::Lighten => Text::new("Lighten").content(),
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
        brightness: Entity<SliderWithField>,
        saturation: Entity<SliderWithField>,
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
                .on_select_changed(move |new, window, app| {
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
                            this.sync(window, cx);
                        }
                    });
                })
        });
        MergeMode::LayerBlend { blend_mode }
    }
}

impl InspectorMode {
    fn sync_mode(&mut self, window: &mut Window, node_kind: NodeKind, cx: &mut Context<Inspector>) {
        match node_kind {
            NodeKind::Normal {
                brightness: new_brightness,
                saturation: new_saturation,
                ..
            } => {
                if let InspectorMode::Normal {
                    brightness,
                    saturation,
                } = self
                {
                    brightness.update(cx, |field, cx| field.set_value(new_brightness, window, cx));
                    saturation.update(cx, |field, cx| field.set_value(new_saturation, window, cx));
                } else {
                    let brightness = new_slider_field(
                        "Brightness",
                        new_brightness,
                        SliderRange {
                            min: 0.0,
                            max: 2.0,
                            step: 0.01,
                        },
                        NodeAttrDiff::Brightness,
                        window,
                        cx,
                    );
                    let saturation = new_slider_field(
                        "Saturation",
                        new_saturation,
                        SliderRange {
                            min: 0.0,
                            max: 2.0,
                            step: 0.01,
                        },
                        NodeAttrDiff::Saturation,
                        window,
                        cx,
                    );
                    *self = InspectorMode::Normal {
                        brightness,
                        saturation,
                    };
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
    pub fn new(window: &mut Window, cx: &mut Context<Self>, edit_state: Entity<EditState>) -> Self {
        cx.subscribe_in(
            &edit_state,
            window,
            |this, _edit_state, _event: &state::SessionChanged, window, cx| {
                this.sync(window, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            &edit_state,
            window,
            |this, _edit_state, _event: &state::SelectedNodeChanged, window, cx| {
                this.sync(window, cx);
            },
        )
        .detach();
        cx.subscribe_in(
            &edit_state,
            window,
            |_this, _edit_state, event: &state::NodeAttrChanged, _window, cx| {
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
        inspector.sync(window, cx);
        inspector
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        self.mode.sync_mode(window, node.kind.clone(), cx);
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
                        } => container
                            .child(brightness.clone())
                            .child(saturation.clone())
                            .into_any_element(),
                        InspectorMode::Merge { mode } => {
                            let container = container.child(single_line_item(
                                "Use Layer Blender",
                                CheckBox::new("use-layer-blende-mode")
                                    .when(mode.layer_blend(), |check_box| check_box.select())
                                    .on_change(move |enabled, window, app| {
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
                                                    "Failed to apply merge mode changes: {:#}",
                                                    e
                                                );
                                                this.sync(window, cx);
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

fn new_slider_field(
    label: &'static str,
    value: f32,
    range: SliderRange,
    on_diff: fn(f32) -> NodeAttrDiff,
    window: &mut Window,
    cx: &mut Context<Inspector>,
) -> Entity<SliderWithField> {
    let this = cx.weak_entity();
    cx.new(move |cx| {
        SliderWithField::new(
            label,
            value,
            range,
            move |value, window, app| {
                let _ = this.update(app, |this, cx| {
                    let Some(node_id) = this.edit_state.read(cx).get_selected_node() else {
                        return;
                    };
                    let diff = WorkflowDiff {
                        payload: WorkflowDiffPayload::NodeAttr(node_id, on_diff(value)),
                    };
                    let edit_state = this.edit_state.clone();
                    let id = cx.entity_id();
                    if let Err(e) = edit_state.update(cx, |this, cx| this.update(diff, id, cx)) {
                        warn!("Failed to apply {} changes: {:#}", label, e);
                        this.sync(window, cx);
                    }
                });
            },
            window,
            cx,
        )
    })
}

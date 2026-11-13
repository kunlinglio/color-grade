use std::sync::Arc;

use gpui_kit::base::slider::{SliderEvent, SliderValue};
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::media::media_type::Media;
use crate::project::{
    NodeId, NodeKind, NodeKindDiff, ProjectRuntimeHandler, WorkFlowDiff, WorkFlowDiffPayload,
};
use crate::ui::components::{Slider, Text};

/// Color grading panel
pub struct Inspector {
    runtime: ProjectRuntimeHandler,
    brightness: Entity<SliderState>,
    saturation: Entity<SliderState>,
    selected_node: Entity<Option<NodeId>>,
    /// Current state based on version of workflow
    version: Option<u64>,
}

impl Inspector {
    pub fn new(
        cx: &mut Context<Self>,
        runtime: ProjectRuntimeHandler,
        selected_media: Entity<Option<Arc<Media>>>,
        selected_node: Entity<Option<NodeId>>,
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

        cx.subscribe(&brightness, |this, _state, event, cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event {
                let Some(version) = this.version else {
                    return;
                };
                let Some(node_id) = *this.selected_node.read(cx) else {
                    return;
                };
                let diff = WorkFlowDiff {
                    version: version,
                    payload: WorkFlowDiffPayload::NodeAttr(
                        node_id,
                        NodeKindDiff::Brightness(*value),
                    ),
                };
                this.runtime.update_state(diff);
                // The runtime bumps its version on every accepted diff, so keep
                // our cached copy in step or the next diff will look outdated.
                // FIXME: Come up with a more reliable solution
                this.version = Some(version + 1);
            }
        })
        .detach();

        cx.subscribe(&saturation, |this, _state, event, cx| {
            if let SliderEvent::Change(SliderValue::Single(value)) = event {
                let Some(version) = this.version else {
                    return;
                };
                let Some(node_id) = *this.selected_node.read(cx) else {
                    return;
                };
                let diff = WorkFlowDiff {
                    version: version,
                    payload: WorkFlowDiffPayload::NodeAttr(
                        node_id,
                        NodeKindDiff::Saturation(*value),
                    ),
                };
                this.runtime.update_state(diff);
                // The runtime bumps its version on every accepted diff, so keep
                // our cached copy in step or the next diff will look outdated.
                // FIXME: Come up with a more reliable solution
                this.version = Some(version + 1);
            }
        })
        .detach();

        cx.observe(&selected_media, |this, selected, cx| {
            if selected.read(cx).is_none() {
                // TODO: Handle effect when select nothing
                return;
            }
            this.update_state(cx);
        })
        .detach();

        cx.observe(&selected_node, |this, selected, cx| {
            if selected.read(cx).is_none() {
                // TODO: Handle effect when select nothing
                return;
            }
            this.update_state(cx);
        })
        .detach();

        Inspector {
            runtime,
            brightness,
            saturation,
            selected_node,
            version: None,
        }
    }

    fn update_state(&self, cx: &mut Context<Self>) {
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let workflow = runtime.get_state().await;
            let _ = this.update(cx, |this, cx| {
                let Some(node_id) = *this.selected_node.read(cx) else {
                    return;
                };
                let Some(node) = workflow.nodes.iter().find(|node| node.node_id == node_id) else {
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
                this.version = Some(workflow.version);
                cx.notify()
            });
        })
        .detach();
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

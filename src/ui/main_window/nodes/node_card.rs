use gpui_kit::assets::IconName;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::project::{
    Node, NodeId, NodeKind, WorkflowDiff, WorkflowDiffPayload, WorkflowTopoChange,
};
use crate::ui::components::{Icon, Outline, Text};
use crate::ui::main_window::state::EditState;
use crate::ui::{Theme, get_theme};

use super::hud::{NodePosHud, PosInfo};
use super::{NODES_PADDING, Nodes, PORT_SIZE};

const NODE_WIDTH: f32 = 120.;
const NODE_HEIGHT: f32 = 60.;

const NODE_ADD_GAP: f32 = 32.;
const NODE_ACTIONS_GROUP: &str = "node-actions";

pub(super) type PortId = usize;

pub(super) struct NodeCard {
    id: NodeId,
    father: WeakEntity<Nodes>,
    selected: bool,
    pos: (f32, f32),
    kind: NodeCardKind,
    /// Used to notify select and pos changed
    edit_state: Entity<EditState>,
    /// States to support drag
    drag: DragState,
    /// Weak ref to Hud for update
    pos_hud: WeakEntity<NodePosHud>,
}

enum DragState {
    Empty,
    Dragging {
        start_pos: (f32, f32),
        start_cursor: Point<Pixels>,
    },
}

enum NodeCardKind {
    Normal {},
    Merge { input_count: usize },
    Generator {},
}

impl NodeCard {
    pub(super) fn new(
        node: &Node,
        edit_state: Entity<EditState>,
        selected: bool,
        pos_hud: WeakEntity<NodePosHud>,
        father: WeakEntity<Nodes>,
    ) -> Self {
        let kind = match &node.kind {
            NodeKind::Normal { .. } => NodeCardKind::Normal {},
            NodeKind::Merge { sources, .. } => NodeCardKind::Merge {
                input_count: sources.len(),
            },
            NodeKind::Generator { .. } => NodeCardKind::Generator {},
        };
        Self {
            id: node.node_id,
            selected: selected,
            pos: node.pos,
            kind,
            edit_state,
            drag: DragState::Empty,
            pos_hud,
            father,
        }
    }
    pub(super) fn sync(&mut self, node: &Node, selected: bool) {
        self.selected = selected;
        // To avoid sync interrupt dragging
        if !matches!(self.drag, DragState::Dragging { .. }) {
            self.pos = node.pos;
        }
        self.kind = match &node.kind {
            NodeKind::Normal { .. } => NodeCardKind::Normal {},
            NodeKind::Merge { sources, .. } => NodeCardKind::Merge {
                input_count: sources.len(),
            },
            NodeKind::Generator { .. } => NodeCardKind::Generator {},
        };
    }
    pub(super) fn set_select(&mut self, select: bool) {
        self.selected = select;
    }
    fn drag_move(&mut self, event: &DragMoveEvent<NodeId>, cx: &mut Context<Self>) {
        let cur_cursor = event.event.position;
        let DragState::Dragging {
            start_pos,
            start_cursor,
        } = &mut self.drag
        else {
            return;
        };
        let target_pos = (
            start_pos.0 + f32::from(cur_cursor.x - start_cursor.x),
            start_pos.1 + f32::from(cur_cursor.y - start_cursor.y),
        );
        self.pos = self.clamp_pos(target_pos, cx);
        if let Some(hud) = &mut self.pos_hud.upgrade() {
            hud.update(cx, |this, cx| {
                this.update(
                    cx,
                    PosInfo {
                        x: self.pos.0,
                        y: self.pos.1,
                    },
                );
            })
        }
        cx.notify();
    }

    fn clamp_pos(&self, target: (f32, f32), cx: &mut Context<Self>) -> (f32, f32) {
        let size = self
            .father
            .upgrade()
            .expect("Father should not quit")
            .read(cx)
            .size();
        let max_x = (size.0 - NODE_WIDTH - NODES_PADDING).max(NODES_PADDING);
        let max_y = (size.1 - NODE_HEIGHT - NODES_PADDING).max(NODES_PADDING);
        (
            target.0.clamp(NODES_PADDING, max_x),
            target.1.clamp(NODES_PADDING, max_y),
        )
    }

    fn add_serial_node(&mut self, cx: &mut Context<Self>) {
        let pos = self.clamp_pos((self.pos.0 + NODE_WIDTH + NODE_ADD_GAP, self.pos.1), cx);
        let diff = WorkflowDiff {
            payload: WorkflowDiffPayload::Topology(WorkflowTopoChange::AddSerialNode {
                from: self.id,
                pos,
            }),
        };
        let father_id = self.father_id();
        if let Err(error) = self
            .edit_state
            .update(cx, |state, cx| state.update(diff, father_id, cx))
        {
            warn!("Failed to add serial node: {error:#}");
        }
    }

    fn add_parallel_node(&mut self, cx: &mut Context<Self>) {
        let pos = self.clamp_pos((self.pos.0, self.pos.1 + NODE_HEIGHT + NODE_ADD_GAP), cx);
        let diff = WorkflowDiff {
            payload: WorkflowDiffPayload::Topology(WorkflowTopoChange::AddParreleNode {
                from: self.id,
                pos,
                node_width: NODE_WIDTH,
            }),
        };
        let father_id = self.father_id();
        if let Err(error) = self
            .edit_state
            .update(cx, |state, cx| state.update(diff, father_id, cx))
        {
            warn!("Failed to add parallel node: {error:#}");
        }
    }

    fn drag_finish(&mut self, cx: &mut Context<Self>) {
        let DragState::Dragging { start_pos, .. } = self.drag else {
            return;
        };
        self.drag = DragState::Empty;
        if let Some(hud) = &mut self.pos_hud.upgrade() {
            hud.update(cx, |this, cx| {
                this.clear(cx);
            })
        }
        if self.pos == start_pos {
            return;
        }
        let diff = WorkflowDiff {
            payload: WorkflowDiffPayload::NodePos(self.id, self.pos),
        };
        let father_id = self.father_id();
        if let Err(e) = self
            .edit_state
            .update(cx, |state, cx| state.update(diff, father_id, cx))
        {
            self.pos = start_pos;
            warn!("Failed to update workflow: {:#}", e);
            return;
        };
    }

    fn father_id(&self) -> EntityId {
        self.father
            .upgrade()
            .expect("Father should not quit")
            .entity_id()
    }

    fn input_count(&self) -> usize {
        match self.kind {
            NodeCardKind::Normal { .. } => 1,
            NodeCardKind::Merge { input_count } => input_count,
            NodeCardKind::Generator {} => 0,
        }
    }
    pub(super) fn output_anchor(&self, relative: bool) -> Point<Pixels> {
        let relative_pos = point(px(NODE_WIDTH), px(NODE_HEIGHT / 2.0));
        if relative {
            relative_pos
        } else {
            relative_pos + point(px(self.pos.0), px(self.pos.1))
        }
    }

    pub(super) fn input_anchor(&self, port: PortId, relative: bool) -> Point<Pixels> {
        let count = match self.kind {
            NodeCardKind::Normal {} => 1,
            NodeCardKind::Merge { input_count } => input_count,
            NodeCardKind::Generator {} => 0,
        };
        let y = if count <= 1 {
            NODE_HEIGHT / 2.0
        } else {
            NODE_HEIGHT * (port as f32 + 1.0) / (count as f32 + 1.0)
        };
        let relative_pos = point(px(0.0), px(y));
        if relative {
            relative_pos
        } else {
            relative_pos + point(px(self.pos.0), px(self.pos.1))
        }
    }
}

impl Render for NodeCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let add_button = |id: SharedString| {
            div()
                .id(id)
                .absolute()
                .size(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(theme.colors.background.elevated)
                .border_1()
                .border_color(theme.colors.border.subtle)
                .invisible()
                .group_hover(NODE_ACTIONS_GROUP, |style| style.visible())
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                    cx.stop_propagation();
                })
                .child(
                    Icon::new(IconName::Plus)
                        .size(px(16.0))
                        .text_color(theme.colors.text.primary),
                )
        };
        div()
            .id(format!("node-card-{}", self.id))
            .group(NODE_ACTIONS_GROUP)
            .absolute()
            .left(px(self.pos.0))
            .top(px(self.pos.1))
            .w(px(NODE_WIDTH))
            .h(px(NODE_HEIGHT))
            .rounded(theme.radii.medium)
            .shadow_lg()
            .bg(theme.colors.background.elevated)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    this.drag = DragState::Dragging {
                        start_pos: this.pos,
                        start_cursor: event.position,
                    };
                    this.edit_state.update(cx, |state, cx| {
                        state.select_node(Some(this.id), this.father_id(), cx);
                    });
                    cx.stop_propagation();
                }),
            )
            // we do not need any drag ghost, so construct a empty entity
            .on_drag(self.id, |_id, _point, _window, cx| cx.new(|_| Empty))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<NodeId>, _window, cx| {
                    this.drag_move(event, cx);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    this.drag_finish(cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _event, _window, cx| this.drag_finish(cx)),
            )
            .when(self.selected, |card| {
                card.child(
                    Outline::new(px(1.5), px(2.5))
                        .color(theme.colors.accent)
                        .radius(theme.radii.medium),
                )
            })
            .child(Text::new("Nodes").content())
            .children((0..self.input_count()).map(|port_id| {
                let mut center = self.input_anchor(port_id, true);
                center.x = px(0.);
                render_port(center, &theme)
            }))
            .child({
                let center = self.output_anchor(true);
                render_port(center, &theme)
            })
            .child(
                add_button(format!("add-serial-{}", self.id).into())
                    .right(px(-10.0))
                    .top(px(NODE_HEIGHT / 2.0 - 10.0))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        cx.stop_propagation();
                        this.add_serial_node(cx);
                    })),
            )
            .when(matches!(&self.kind, NodeCardKind::Normal {}), |card| {
                card.child(
                    add_button(format!("add-parallel-{}", self.id).into())
                        .bottom(px(-10.0))
                        .left(px(NODE_WIDTH / 2.0 - 10.0))
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            cx.stop_propagation();
                            this.add_parallel_node(cx);
                        }))
                        .into_any_element(),
                )
            })
    }
}

pub(super) fn render_port(center: Point<Pixels>, theme: &Theme) -> impl IntoElement {
    div()
        .absolute()
        .left(center.x - px(PORT_SIZE / 2.0))
        .top(center.y - px(PORT_SIZE / 2.0))
        .size(px(PORT_SIZE))
        .rounded_full()
        .bg(theme.colors.background.inverse)
}

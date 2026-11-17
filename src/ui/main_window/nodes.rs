use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::*;

use crate::project::{Node, NodeId, NodeKind, Source, WorkFlowDiff, WorkFlowDiffPayload};
use crate::ui::components::{Outline, ResizeSensitive, Text};
use crate::ui::get_theme;
use crate::ui::main_window::state::{self, EditState};

const NODES_PADDING: f32 = 16.;

pub struct Nodes {
    edit_state: Entity<EditState>,
    state: NodesState,
    size: Rc<RefCell<(f32, f32)>>,
}

enum NodesState {
    Inactive,
    Active {
        /// Cached node cards, to avoid query workflow and rebuild everything
        /// from scratch every render pass.
        node_cards: HashMap<NodeId, Entity<NodeCard>>,
        edge_cache: HashMap<(Source, NodeId, PortId), Entity<Edge>>,
        last_edge: Option<Entity<Edge>>,
    },
}

impl Nodes {
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
            |this, _edit_state, event: &state::SelectedNodeChanged, cx| {
                let state::SelectedNodeChanged {
                    previous,
                    current,
                    origin,
                } = event;
                let id = cx.entity_id();
                assert_eq!(
                    id, *origin,
                    "The selected node should only be changed by nodes"
                );
                let NodesState::Active { node_cards, .. } = &mut this.state else {
                    return;
                };
                if let Some(previous) = previous {
                    if let Some(card) = node_cards.get(previous) {
                        card.update(cx, |this, _cx| {
                            this.selected = false;
                        })
                    }
                }
                if let Some(current) = current {
                    if let Some(card) = node_cards.get(current) {
                        card.update(cx, |this, _cx| {
                            this.selected = true;
                        })
                    }
                }
            },
        )
        .detach();
        cx.subscribe(
            &edit_state,
            |_this, _edit_state, event: &state::NodePosChanged, cx| {
                let id = cx.entity_id();
                assert_eq!(
                    id, event.origin,
                    "The node position should only be changed by nodes"
                )
                // nothing to do, the node card will update its own select state
            },
        )
        .detach();
        cx.subscribe(
            &edit_state,
            |_this, _edit_state, _event: &state::WorkflowTopoChanged, _cx| {
                todo!("Implement workflow topology change")
            },
        )
        .detach();
        Nodes {
            edit_state,
            state: NodesState::Inactive,
            size: Rc::new(RefCell::new((0., 0.))),
        }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let edit_state = self.edit_state.read(cx);
        let selected_node = edit_state.get_selected_node();
        match edit_state.get_workflow() {
            Some(workflow) => {
                let nodes: Vec<_> = workflow.nodes().iter().map(|node| node.clone()).collect();
                let output = workflow.output();
                if let NodesState::Inactive = &self.state {
                    self.state = NodesState::Active {
                        node_cards: HashMap::new(),
                        edge_cache: HashMap::new(),
                        last_edge: None,
                    };
                }
                let NodesState::Active {
                    node_cards,
                    edge_cache,
                    last_edge,
                } = &mut self.state
                else {
                    unreachable!("Inactive state must have been normalized above");
                };
                let mut outdated: HashSet<_> = node_cards.keys().copied().collect();
                // update node cards
                for node in &nodes {
                    let id = node.node_id;
                    let selected = Some(id) == selected_node;
                    match node_cards.get_mut(&id) {
                        Some(card) => {
                            card.update(cx, |this, cx| {
                                this.sync(node, selected);
                                cx.notify();
                            });
                            outdated.remove(&id);
                        }
                        None => {
                            let entity_id = cx.entity_id();
                            let card = cx.new(|_cx| {
                                NodeCard::new(
                                    &node,
                                    self.edit_state.clone(),
                                    selected,
                                    entity_id,
                                    self.size.clone(),
                                )
                            });
                            node_cards.insert(id, card.clone());
                        }
                    };
                }
                for id in outdated {
                    node_cards.remove(&id);
                }
                // update edge cache
                let mut outdated: HashSet<_> = edge_cache.keys().copied().collect();
                for node in &nodes {
                    let id = node.node_id;
                    let to_be_handled = match &node.kind {
                        NodeKind::Normal {
                            source: Some(source),
                            ..
                        } => {
                            let port = 0;
                            vec![(port, source)]
                        }
                        NodeKind::Merge { sources, .. } => sources
                            .iter()
                            .enumerate()
                            .flat_map(|(port, source)| {
                                let Some(source) = source else {
                                    return None;
                                };
                                Some((port, source))
                            })
                            .collect(),
                        _ => {
                            vec![]
                        }
                    };
                    for (port, source) in to_be_handled {
                        let to_node = node_cards
                            .get(&id)
                            .expect("Node has update before")
                            .downgrade();
                        let to = EdgeTo::Node(port, to_node);
                        match edge_cache.get_mut(&(*source, id, port)) {
                            Some(edge) => {
                                edge.update(cx, |this, cx| {
                                    *this =
                                        Edge::new(cx, source, node_cards, to, self.size.clone());
                                    cx.notify();
                                });
                                outdated.remove(&(*source, id, port));
                            }
                            None => {
                                let edge = cx.new(|cx| {
                                    Edge::new(cx, source, node_cards, to, self.size.clone())
                                });
                                edge_cache.insert((*source, id, port), edge);
                            }
                        }
                    }
                }
                for id in outdated {
                    edge_cache.remove(&id);
                }
                // handle last edge
                if let Some(output) = output {
                    let to = EdgeTo::Output;
                    match last_edge {
                        Some(edge) => {
                            edge.update(cx, |this, cx| {
                                *this = Edge::new(cx, &output, node_cards, to, self.size.clone());
                                cx.notify();
                            });
                        }
                        None => {
                            let edge = cx.new(|cx| {
                                Edge::new(cx, &output, node_cards, to, self.size.clone())
                            });
                            *last_edge = Some(edge)
                        }
                    }
                } else {
                    *last_edge = None;
                }
            }
            None => {
                self.state = NodesState::Inactive;
            }
        };
        cx.notify();
    }
}

impl Render for Nodes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        div().size_full().child(
            ResizeSensitive::new()
                .size_full()
                .on_resize(move |bounds, _window, app| {
                    let _ = this.update(app, |this, _cx| {
                        let width = f32::from(bounds.size.width);
                        let height = f32::from(bounds.size.height);
                        *this.size.borrow_mut() = (width, height);
                    });
                })
                .child(Text::new("Nodes").title())
                .child(match &self.state {
                    NodesState::Inactive => Text::new("Select a Media to active nodes")
                        .content()
                        .into_any_element(),
                    NodesState::Active {
                        node_cards,
                        edge_cache,
                        last_edge,
                    } => div()
                        .id("nodes-container")
                        .relative()
                        .size_full()
                        .overflow_hidden()
                        .children(edge_cache.values().cloned())
                        .children(last_edge.iter().cloned())
                        .children(node_cards.values().cloned())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _event, _window, cx| {
                                let id = cx.entity_id();
                                this.edit_state
                                    .update(cx, |state, cx| state.select_node(None, id, cx));
                            }),
                        )
                        .into_any_element(),
                }),
        )
    }
}

type PortId = usize;

struct Edge {
    from: EdgeFrom,
    to: EdgeTo,
    nodes_size: Rc<RefCell<(f32, f32)>>,
    _subscriptions: Vec<Subscription>,
}

enum EdgeFrom {
    Node(WeakEntity<NodeCard>),
    Input,
}

enum EdgeTo {
    Node(PortId, WeakEntity<NodeCard>),
    Output,
}

impl Edge {
    fn new(
        cx: &mut Context<Self>,
        source: &Source,
        node_cards: &HashMap<NodeId, Entity<NodeCard>>,
        to: EdgeTo,
        nodes_size: Rc<RefCell<(f32, f32)>>,
    ) -> Self {
        let from = match source {
            Source::Input => EdgeFrom::Input,
            Source::Node(id) => {
                let node = node_cards
                    .get(&id)
                    .expect("Node has update before")
                    .downgrade();
                EdgeFrom::Node(node)
            }
        };
        let mut _subscriptions = Vec::new();
        if let EdgeFrom::Node(card) = &from {
            if let Some(card) = card.upgrade() {
                _subscriptions.push(cx.observe(&card, |_, _, cx| cx.notify()));
            }
        }
        if let EdgeTo::Node(_, card) = &to {
            if let Some(card) = card.upgrade() {
                _subscriptions.push(cx.observe(&card, |_, _, cx| cx.notify()));
            }
        }
        Self {
            from,
            to,
            _subscriptions,
            nodes_size,
        }
    }
    fn anchors(&self, cx: &App) -> Option<(Point<Pixels>, Point<Pixels>)> {
        let from = match &self.from {
            EdgeFrom::Input => point(px(NODES_PADDING), px(self.nodes_size.borrow().1 / 2.)), // Input port
            EdgeFrom::Node(card) => card.upgrade()?.read(cx).output_anchor(),
        };
        let to = match &self.to {
            EdgeTo::Output => point(
                px(self.nodes_size.borrow().0 - NODES_PADDING),
                px(self.nodes_size.borrow().1 / 2.),
            ), // Output port
            EdgeTo::Node(port, card) => card.upgrade()?.read(cx).input_anchor(*port),
        };
        Some((from, to))
    }
}

impl Render for Edge {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let Some((from, to)) = self.anchors(cx) else {
            return div();
        };

        let fx = f32::from(from.x);
        let fy = f32::from(from.y);
        let tx = f32::from(to.x);
        let ty = f32::from(to.y);

        let left = fx.min(tx);
        let top = fy.min(ty);
        let width = (tx - fx).abs().max(1.0);
        let height = (ty - fy).abs().max(1.0);
        let color = theme.colors.text.muted;

        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(width))
            .h(px(height))
            .child(
                canvas(
                    |_bounds, _window, _cx| (),
                    move |bounds, _, window, _cx| {
                        let from = point(
                            px(fx - left) + bounds.origin.x,
                            px(fy - top) + bounds.origin.y,
                        );
                        let to = point(
                            px(tx - left) + bounds.origin.x,
                            px(ty - top) + bounds.origin.y,
                        );

                        let mut builder = PathBuilder::stroke(px(1.5));
                        builder.move_to(from);
                        builder.line_to(to);
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, color);
                        }
                    },
                )
                .size_full(),
            )
    }
}

const NODE_WIDTH: f32 = 160.;
const NODE_HEIGHT: f32 = 72.;

struct NodeCard {
    id: NodeId,
    nodes_entity_id: EntityId,
    selected: bool,
    pos: (f32, f32),
    kind: NodeCardKind,
    /// Used to notify select and pos changed
    edit_state: Entity<EditState>,
    /// States to support drag
    drag: DragState,
    nodes_size: Rc<RefCell<(f32, f32)>>,
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
    fn new(
        node: &Node,
        edit_state: Entity<EditState>,
        selected: bool,
        nodes_entity_id: EntityId,
        nodes_size: Rc<RefCell<(f32, f32)>>,
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
            nodes_entity_id,
            selected: selected,
            pos: node.pos,
            kind,
            edit_state,
            drag: DragState::Empty,
            nodes_size,
        }
    }
    fn sync(&mut self, node: &Node, selected: bool) {
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
    fn drag_move(&mut self, event: &DragMoveEvent<NodeId>, cx: &mut Context<Self>) {
        let position = event.event.position;
        let DragState::Dragging {
            start_pos,
            start_cursor,
        } = &mut self.drag
        else {
            return;
        };
        let target = (
            start_pos.0 + f32::from(position.x - start_cursor.x),
            start_pos.1 + f32::from(position.y - start_cursor.y),
        );
        let size = self.nodes_size.borrow();
        self.pos = Self::clamp_towards((size.0 / 2., size.1 / 2.), target, *size);
        cx.notify();
    }
    fn clamp_towards(anchor: (f32, f32), target: (f32, f32), bounds: (f32, f32)) -> (f32, f32) {
        let (w, h) = (f32::from(bounds.0), f32::from(bounds.1));
        if w <= 0.0 || h <= 0.0 {
            return target;
        }
        let max_x = (w - NODE_WIDTH).max(0.0);
        let max_y = (h - NODE_HEIGHT).max(0.0);

        let anchor = (anchor.0.clamp(0.0, max_x), anchor.1.clamp(0.0, max_y));
        let (dx, dy) = (target.0 - anchor.0, target.1 - anchor.1);

        let mut t = 1.0f32;
        if dx > 0.0 {
            t = t.min((max_x - anchor.0) / dx);
        } else if dx < 0.0 {
            t = t.min((0.0 - anchor.0) / dx);
        }
        if dy > 0.0 {
            t = t.min((max_y - anchor.1) / dy);
        } else if dy < 0.0 {
            t = t.min((0.0 - anchor.1) / dy);
        }
        let t = t.clamp(0.0, 1.0);

        (anchor.0 + dx * t, anchor.1 + dy * t)
    }

    fn drag_finish(&mut self, cx: &mut Context<Self>) {
        let DragState::Dragging { start_pos, .. } = self.drag else {
            return;
        };
        self.drag = DragState::Empty;
        let diff = WorkFlowDiff {
            payload: WorkFlowDiffPayload::NodePos(self.id, self.pos),
        };
        if let Err(e) = self
            .edit_state
            .update(cx, |state, cx| state.update(diff, self.nodes_entity_id, cx))
        {
            self.pos = start_pos;
            warn!("Failed to update workflow: {:#}", e);
            return;
        };
    }
    fn output_anchor(&self) -> Point<Pixels> {
        point(
            px(self.pos.0 + NODE_WIDTH),
            px(self.pos.1 + NODE_HEIGHT / 2.0),
        )
    }

    fn input_anchor(&self, port: PortId) -> Point<Pixels> {
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
        point(px(self.pos.0), px(self.pos.1 + y))
    }
}

impl Render for NodeCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let element = div()
            .id(format!("node-card-{}", self.id))
            .absolute()
            .left(px(self.pos.0))
            .top(px(self.pos.1))
            .w(px(NODE_WIDTH))
            .h(px(NODE_HEIGHT))
            .rounded(theme.radii.medium)
            .bg(theme.colors.background.surface)
            .border_1()
            .border_color(theme.colors.border.subtle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    this.drag = DragState::Dragging {
                        start_pos: this.pos,
                        start_cursor: event.position,
                    };
                    this.edit_state.update(cx, |state, cx| {
                        state.select_node(Some(this.id), this.nodes_entity_id, cx);
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
            );
        let element = if self.selected {
            element.child(
                Outline::new(px(1.), px(3.))
                    .color(theme.colors.focus)
                    .radius(theme.radii.medium),
            )
        } else {
            element
        };
        element.child(Text::new("Node").content())
    }
}

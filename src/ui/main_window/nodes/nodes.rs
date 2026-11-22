use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::base::StyledExt;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::project::{NodeId, NodeKind, Source};
use crate::ui::components::{ResizeSensitive, Text};
use crate::ui::main_window::state::{self, EditState};

use super::edge::{Edge, EdgeTo};
use super::hud::NodePosHud;
use super::node_card::{NodeCard, PortId};

pub struct Nodes {
    edit_state: Entity<EditState>,
    state: NodesState,
    size: Rc<RefCell<(f32, f32)>>,
    pos_hud: Entity<NodePosHud>,
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
                        card.update(cx, |this, cx| {
                            this.set_select(false);
                            cx.notify();
                        })
                    }
                }
                if let Some(current) = current {
                    if let Some(card) = node_cards.get(current) {
                        card.update(cx, |this, cx| {
                            this.set_select(true);
                            cx.notify();
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
            |this, _edit_state, event: &state::WorkflowTopoChanged, cx| {
                assert_eq!(
                    cx.entity_id(),
                    event.origin,
                    "The workflow topology should only be changed by nodes"
                );
                this.sync(cx);
            },
        )
        .detach();
        let pos_hud = cx.new(|_cx| NodePosHud::new());
        Nodes {
            edit_state,
            state: NodesState::Inactive,
            size: Rc::new(RefCell::new((0., 0.))),
            pos_hud,
        }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let edit_state = self.edit_state.read(cx);
        let selected_node = edit_state.get_selected_node();
        match edit_state.get_workflow() {
            None => {
                self.state = NodesState::Inactive;
            }
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
                            let hud = self.pos_hud.downgrade();
                            let card = cx.new(|_cx| {
                                NodeCard::new(
                                    &node,
                                    self.edit_state.clone(),
                                    selected,
                                    entity_id,
                                    self.size.clone(),
                                    hud,
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
        };
        cx.notify();
    }
}

impl Render for Nodes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        div()
            .size_full()
            .v_flex()
            .child(Text::new("Nodes").title())
            .child(
                ResizeSensitive::new()
                    .flex_1()
                    .on_resize(move |bounds, _window, app| {
                        let _ = this.update(app, |this, _cx| {
                            let width = f32::from(bounds.size.width);
                            let height = f32::from(bounds.size.height);
                            *this.size.borrow_mut() = (width, height);
                        });
                    })
                    .child(match &self.state {
                        NodesState::Inactive => div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Text::new("Select a Media to active nodes").content())
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
            .child(self.pos_hud.clone())
    }
}

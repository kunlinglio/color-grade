use std::collections::{HashMap, HashSet};

use gpui_kit::*;

use crate::project::{Node, NodeId, NodeKind, Source};
use crate::ui::components::Text;
use crate::ui::get_theme;
use crate::ui::main_window::state::EditState;

pub struct Nodes {
    edit_state: Entity<EditState>,
    state: NodesState,
}

enum NodesState {
    Inactive,
    Active {
        /// Cached node cards, to avoid query workflow and rebuild everything
        /// from scratch every render pass.
        node_cards: HashMap<NodeId, Entity<NodeCard>>,
        edge_cache: HashSet<Edge>,
    },
}

type PortId = usize;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Edge {
    from: Source,
    to: (NodeId, PortId),
}

impl Edge {
    fn collect(nodes: &[Node]) -> HashSet<Self> {
        let live: HashSet<NodeId> = nodes.iter().map(|node| node.node_id).collect();
        let mut edges = HashSet::new();

        for node in nodes {
            let to = node.node_id;
            match &node.kind {
                NodeKind::Normal {
                    source: Some(source),
                    ..
                } => {
                    let edge = Edge {
                        from: *source,
                        to: (to, 0),
                    };
                    if edge.is_live(&live) {
                        edges.insert(edge);
                    }
                }
                NodeKind::Merge { sources, .. } => {
                    for (port, source) in sources.iter().enumerate() {
                        let Some(source) = source else { continue };
                        let edge = Edge {
                            from: *source,
                            to: (to, port),
                        };
                        if edge.is_live(&live) {
                            edges.insert(edge);
                        }
                    }
                }
                _ => {}
            }
        }

        edges
    }

    fn is_live(&self, live: &HashSet<NodeId>) -> bool {
        match self.from {
            Source::Input => true,
            Source::Node(id) => live.contains(&id),
        }
    }
}

impl Nodes {
    pub fn new(cx: &mut Context<Self>, edit_state: Entity<EditState>) -> Self {
        cx.observe(&edit_state, |this, _entity, cx| {
            this.sync(cx);
        })
        .detach();
        Nodes {
            edit_state,
            state: NodesState::Inactive,
        }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let edit_state = self.edit_state.read(cx);
        match edit_state.get_workflow() {
            Some(workflow) => {
                let nodes: Vec<_> = workflow.nodes().iter().map(|node| node.clone()).collect();
                if let NodesState::Inactive = &self.state {
                    self.state = NodesState::Active {
                        node_cards: HashMap::new(),
                        edge_cache: HashSet::new(),
                    };
                }
                let NodesState::Active {
                    node_cards,
                    edge_cache,
                } = &mut self.state
                else {
                    unreachable!("Inactive state must have been normalized above");
                };
                // rerbuild node cards
                let mut outdated: HashSet<_> = node_cards.keys().copied().collect();
                for node in &nodes {
                    let id = node.node_id;
                    match node_cards.get_mut(&id) {
                        Some(card) => {
                            card.update(cx, |this, _cx| {
                                this.sync(node);
                            });
                            outdated.remove(&id);
                        }
                        None => {
                            let card = cx.new(|_cx| NodeCard::new(&node));
                            node_cards.insert(id, card);
                        }
                    }
                }
                for id in outdated {
                    node_cards.remove(&id);
                }
                // rebuild edge cache
                *edge_cache = Edge::collect(&nodes);
            }
            None => {
                self.state = NodesState::Inactive;
            }
        };
        cx.notify();
    }
}

impl Render for Nodes {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(Text::new("Nodes").title())
            .child(match &self.state {
                NodesState::Inactive => Text::new("Select a Media to active nodes")
                    .content()
                    .into_any_element(),
                NodesState::Active {
                    node_cards,
                    edge_cache: _,
                } => div()
                    .relative()
                    .size_full()
                    .children(node_cards.values().cloned())
                    .into_any_element(),
            })
    }
}

struct NodeCard {
    selected: bool,
    pos: (f32, f32),
    kind: NodeCardKind,
}

enum NodeCardKind {
    Normal {},
    Merge { input_count: usize },
    Generator {},
}

impl NodeCard {
    fn new(node: &Node) -> Self {
        let kind = match &node.kind {
            NodeKind::Normal { .. } => NodeCardKind::Normal {},
            NodeKind::Merge { sources, .. } => NodeCardKind::Merge {
                input_count: sources.len(),
            },
            NodeKind::Generator { .. } => NodeCardKind::Generator {},
        };
        Self {
            selected: false,
            pos: node.pos,
            kind,
        }
    }
    fn sync(&mut self, node: &Node) {
        self.pos = node.pos;

        self.kind = match &node.kind {
            NodeKind::Normal { .. } => NodeCardKind::Normal {},
            NodeKind::Merge { sources, .. } => NodeCardKind::Merge {
                input_count: sources.len(),
            },
            NodeKind::Generator { .. } => NodeCardKind::Generator {},
        };
    }
}

impl Render for NodeCard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        div()
            .absolute()
            .left(px(self.pos.0))
            .top(px(self.pos.1))
            .w(px(160.0))
            .h(px(72.0))
            .rounded(theme.radii.medium)
            .bg(theme.colors.background.surface)
            .border_1()
            .border_color(theme.colors.border.subtle)
            .child(Text::new("Node").content())
    }
}

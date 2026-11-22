use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::project::{NodeId, Source};
use crate::ui::get_theme;

use super::NODES_PADDING;
use super::node_card::{NodeCard, PortId};

pub(super) struct Edge {
    from: EdgeFrom,
    to: EdgeTo,
    nodes_size: Rc<RefCell<(f32, f32)>>,
    _subscriptions: Vec<Subscription>,
}

enum EdgeFrom {
    Node(WeakEntity<NodeCard>),
    Input,
}

pub(super) enum EdgeTo {
    Node(PortId, WeakEntity<NodeCard>),
    Output,
}

impl Edge {
    pub(super) fn new(
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

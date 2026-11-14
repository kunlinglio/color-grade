use std::sync::Arc;

use gpui_kit::Context;

use crate::media::media_type::Media;
use crate::project::{NodeId, SessionClient, WorkFlow, WorkFlowDiff};

/// NOTE: We didn't have any access control, so this could cause some
/// conflict under high frequent update requests. But I cannot come up
/// with any good enough solutions, so that's a necussary trade off :(.
pub enum EditState {
    Empty,
    Opened {
        client: SessionClient,
        media: Arc<Media>,
        selected_node: Option<NodeId>,
    },
}

impl EditState {
    pub fn new() -> Self {
        Self::Empty
    }
    pub fn get_selected_media(&self) -> Option<Arc<Media>> {
        if let EditState::Opened { media, .. } = self {
            Some(media.clone())
        } else {
            None
        }
    }
    pub fn get_selected_node(&self) -> Option<NodeId> {
        if let EditState::Opened { selected_node, .. } = self {
            *selected_node
        } else {
            None
        }
    }
    pub fn get_workflow(&self) -> Option<Arc<WorkFlow>> {
        if let EditState::Opened { client, .. } = self {
            Some(client.get_workflow())
        } else {
            None
        }
    }
    pub fn open_media(
        &mut self,
        new: Arc<Media>,
        mut client: SessionClient,
        cx: &mut Context<'_, Self>,
    ) {
        let mut updates = client.subscribe_change();
        cx.spawn(async move |this, cx| {
            while let Ok(_) = updates.recv().await {
                let _ = this.update(cx, |_this, cx| cx.notify());
            }
        })
        .detach();

        *self = Self::Opened {
            client,
            media: new,
            selected_node: None,
        };
        cx.notify();
    }
    /// This will block on current thread until it returns if update success.
    /// It worthed, since we need to react to user by update state.
    /// FIXME: Maybe this will cause latency
    pub fn update(&mut self, diff: WorkFlowDiff, cx: &mut Context<Self>) -> bool {
        let changed = match self {
            EditState::Empty => false,
            EditState::Opened { client, .. } => client.update_workflow(diff),
        };
        if changed {
            cx.notify();
        };
        changed
    }
}

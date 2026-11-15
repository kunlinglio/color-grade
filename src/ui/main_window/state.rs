use std::sync::Arc;

use gpui_kit::Context;

use crate::media::media_type::Media;
use crate::project::{NodeId, SessionClient, WorkFlow, WorkFlowDiff};

/// A edit state shared by multiple panel inside mainwindow.
/// Since they all need to edit on same data structure, so a unified
/// enter is necessary to avoid inconsistency.
///
/// NOTE: We didn't have any access control, so this could cause some
/// conflict under high frequent update requests. But I cannot come up
/// with any good enough solutions, so that's a necessary trade off :(.
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
    pub fn get_workflow(&self) -> Option<&WorkFlow> {
        if let EditState::Opened { client, .. } = self {
            Some(client.get_workflow())
        } else {
            None
        }
    }
    pub fn open_media(
        &mut self,
        new: Arc<Media>,
        client: SessionClient,
        cx: &mut Context<'_, Self>,
    ) {
        *self = Self::Opened {
            client,
            media: new,
            selected_node: None,
        };
        cx.notify();
    }
    /// Apply diff to workflow,
    /// returns if the diff validate
    pub fn update(&mut self, diff: WorkFlowDiff, cx: &mut Context<Self>) -> anyhow::Result<()> {
        match self {
            EditState::Empty => anyhow::bail!("No media opened"),
            EditState::Opened { client, .. } => {
                client.update_workflow(diff)?;
                cx.notify();
                Ok(())
            }
        }
    }
}

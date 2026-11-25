use std::sync::Arc;

use gpui_kit::{Context, EntityId, EventEmitter};

use crate::media::Media;
use crate::project::{NodeId, SessionClient, Workflow, WorkflowDiff, WorkflowDiffPayload};

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
    pub fn get_workflow(&self) -> Option<&Workflow> {
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
        origin: EntityId,
        cx: &mut Context<'_, Self>,
    ) {
        *self = Self::Opened {
            client,
            media: new.clone(),
            selected_node: None,
        };
        cx.emit(SessionChanged {
            origin,
            new: Some(new),
        });
    }
    /// Apply diff to workflow,
    /// returns if the diff validate
    pub fn update(
        &mut self,
        diff: WorkflowDiff,
        origin: EntityId,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match self {
            EditState::Empty => anyhow::bail!("No media opened"),
            EditState::Opened { client, .. } => {
                client.update_workflow(diff)?;
                match diff.payload {
                    WorkflowDiffPayload::NodeAttr(..) => {
                        cx.emit(NodeAttrChanged { origin });
                    }
                    WorkflowDiffPayload::NodePos(..) => cx.emit(NodePosChanged { origin }),
                    WorkflowDiffPayload::Topology(_) => {
                        cx.emit(WorkflowTopoChanged { origin });
                    }
                }
                Ok(())
            }
        }
    }
    pub fn select_node(&mut self, id: Option<NodeId>, origin: EntityId, cx: &mut Context<Self>) {
        let EditState::Opened {
            selected_node,
            client,
            ..
        } = self
        else {
            warn!("No media opened, cannot change selected node");
            return;
        };
        if let Some(id) = id {
            if let None = client.get_workflow().get_node(id) {
                warn!("Selected node not exists");
                return;
            }
        }
        if *selected_node == id {
            // not changed
            return;
        }
        let previous = *selected_node;
        *selected_node = id;
        cx.emit(SelectedNodeChanged {
            previous: previous,
            current: id,
            origin,
        });
    }
}

pub struct SelectedNodeChanged {
    pub origin: EntityId,
    pub previous: Option<NodeId>,
    pub current: Option<NodeId>,
}

pub struct SessionChanged {
    pub origin: EntityId,
    pub new: Option<Arc<Media>>,
}

pub struct NodeAttrChanged {
    pub origin: EntityId,
}

pub struct NodePosChanged {
    pub origin: EntityId,
}

pub struct WorkflowTopoChanged {
    pub origin: EntityId,
}

impl EventEmitter<SessionChanged> for EditState {}
impl EventEmitter<SelectedNodeChanged> for EditState {}
impl EventEmitter<NodeAttrChanged> for EditState {}
impl EventEmitter<WorkflowTopoChanged> for EditState {}
impl EventEmitter<NodePosChanged> for EditState {}

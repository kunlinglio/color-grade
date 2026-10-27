//! Defined the communication between the project runtime and UI

use std::path::{Path, PathBuf};

use futures::channel::mpsc;
use futures::channel::oneshot;

use crate::media::comm::MediaSubscription;
use crate::render::FrameStream;
use crate::thumbnail::ThumbnailManagerClient;

/// Communicate handler from the UI and the ProjectRuntime
#[derive(Clone)]
pub struct ProjectRuntimeHandler {
    pub(super) cmd_sender: mpsc::UnboundedSender<Command>,
}

impl ProjectRuntimeHandler {
    pub async fn subscribe_frame_stream(&mut self) -> FrameStream {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::SubscribeFrameStream(tx))
            .expect("Project Runtime thread should not quit");
        let frame_stream = rx.await.expect("Project Runtime thread should not quit");
        frame_stream
    }
    pub fn notify_workflow_changed(&mut self) {
        self.cmd_sender
            .unbounded_send(Command::WorkflowChanged)
            .expect("Project Runtime thread should not quit")
    }
    pub async fn listen_on_media(&mut self) -> MediaSubscription {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::SubscribeMedia(tx))
            .expect("Project Runtime thread should not quit");
        let media_updater = rx.await.expect("Project Runtime thread should not quit");
        media_updater
    }
    pub async fn subscribe_thumbnails(&mut self) -> ThumbnailManagerClient {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::ReqThumbnailClient(tx))
            .expect("Project Runtime thread should not quit");
        rx.await.expect("Project Runtime thread should not quit")
    }
    pub fn open_media(&self, path: &Path) {
        self.cmd_sender
            .unbounded_send(Command::OpenMedia(path.to_path_buf()))
            .expect("Project Runtime thread should not quit")
    }
}

pub enum Command {
    // Request commands: Bringing a Sender
    SubscribeFrameStream(oneshot::Sender<FrameStream>),
    SubscribeMedia(oneshot::Sender<MediaSubscription>),
    ReqThumbnailClient(oneshot::Sender<ThumbnailManagerClient>),
    // Notify commands
    WorkflowChanged,
    /// Open a new media.
    /// NOTE: This will discard old media
    OpenMedia(PathBuf),
}

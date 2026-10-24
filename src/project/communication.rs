//! Defined the communication between the project runtime and UI

use futures::channel::mpsc;
use futures::channel::oneshot;

use crate::media::manager::MediaUpdater;
use crate::render_pipe::FrameStream;

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
            .unwrap();
        let frame_stream = rx.await.expect("Project Runtime should not quit");
        frame_stream
    }
    pub fn notify_workflow_changed(&mut self) {
        self.cmd_sender
            .unbounded_send(Command::WorkflowChanged)
            .unwrap();
    }
    pub async fn request_media(&mut self) -> MediaUpdater {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::RequestMedias(tx))
            .unwrap();
        let media_updater = rx.await.unwrap();
        media_updater
    }
}

pub enum Command {
    // Request commands: Bringing a Sender
    SubscribeFrameStream(oneshot::Sender<FrameStream>),
    RequestMedias(oneshot::Sender<MediaUpdater>),
    // Notify commands
    WorkflowChanged,
}

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use futures::FutureExt;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};
use rayon::ThreadPool;

use crate::media::comm::MediaSubscription;
use crate::media::manager::MediaManager;
use crate::project::session::SessionClient;
use crate::render::Renderer;
use crate::render::{BackendFrame, FrameStream, RenderedFrame, ScopeGraph};
use crate::thumbnail::ThumbnailManager;
use crate::thumbnail::ThumbnailManagerClient;

use super::session::Session;

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    opened: Option<Session>,
    // Services
    renderer: Renderer,
    media_manager: MediaManager,
    thumbnail_manager: ThumbnailManager,
}

impl ProjectRuntime {
    pub fn new(project_path: PathBuf, thread_pool: Arc<ThreadPool>) -> Self {
        let renderer = Renderer::new(thread_pool.clone());
        let media_manager = MediaManager::new(project_path.clone(), thread_pool.clone());
        let thumbnail_manager = ThumbnailManager::new(thread_pool.clone());
        ProjectRuntime {
            opened: None,
            renderer,
            media_manager,
            thumbnail_manager,
        }
    }

    pub fn run(mut self) -> (ProjectRuntimeThread, ProjectRuntimeHandler) {
        let (quit_tx, quit_rx) = oneshot::channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        self.media_manager.issue_scan();
        let handler = thread::Builder::new()
            .name(format!("Project Runtime").to_string())
            .spawn(move || self.main_loop(quit_rx, cmd_rx))
            .expect("Create thread failed!");
        (
            ProjectRuntimeThread {
                handler,
                quit_sender: quit_tx,
            },
            ProjectRuntimeHandler { cmd_sender: cmd_tx },
        )
    }

    fn main_loop(
        &mut self,
        quit_rx: oneshot::Receiver<()>,
        cmd_rx: mpsc::UnboundedReceiver<Command>,
    ) {
        futures::executor::block_on(async {
            let mut quit_rx = quit_rx.fuse();
            let mut cmd_rx = cmd_rx.fuse();
            loop {
                futures::select! {
                    _ = quit_rx => break,
                    command = cmd_rx.next() => {
                        match command {
                            Some(command) => { self.handle_command(command); }
                            None => break,
                        }
                    },
                    _ = self.media_manager.handle_event().fuse() => {},
                    _ = self.thumbnail_manager.handle_event().fuse() => {},
                    _ = async {
                        match &mut self.opened {
                            Some(session) => session.handle_event(&mut self.renderer).await,
                            None => std::future::pending::<()>().await,
                        }
                    }
                    .fuse() => {}
                }
            }
        });
    }

    fn handle_command(&mut self, cmd: Command) {
        // Inside this method, we can ignore all the send/recv failure, since the runtime may lives longer than UI
        match cmd {
            Command::SubscribeFrameStream(tx) => {
                let frame_stream = self.renderer.subscribe();
                let _ = tx.send(frame_stream);
            }
            Command::SubscribeScope(tx) => {
                let scope_stream = self.renderer.subscribe_scope();
                let _ = tx.send(scope_stream);
            }
            Command::RequestScope(from, width, height) => {
                self.renderer.request_scope(from, width, height);
            }
            Command::SubscribeMedia(tx) => {
                let updater = self.media_manager.subscribe();
                let _ = tx.send(updater);
            }
            Command::SubscribeThumbnailClient(tx) => {
                let _ = tx.send(self.thumbnail_manager.client());
            }
            Command::OpenMedia { from, tx } => {
                let Ok((session, session_client)) = Session::new(from.clone()) else {
                    error!("Failed to open media at {}", from.display());
                    todo!("Handle media open failed");
                };
                // Trigger refresh FrameStreams
                session.render(&mut self.renderer);
                self.opened = Some(session);
                let _ = tx.send(session_client);
            }
        }
    }
}

pub struct ProjectRuntimeThread {
    handler: thread::JoinHandle<()>,
    quit_sender: oneshot::Sender<()>, // Only for sending quit command
}

impl ProjectRuntimeThread {
    pub fn quit(self) {
        let _ = self.quit_sender.send(());
        if let Err(_) = self.handler.join() {
            error!("ProjectRuntime thread panicked!")
        }
    }
}

/// Communicate handler from the UI and the ProjectRuntime
#[derive(Clone)]
pub struct ProjectRuntimeHandler {
    cmd_sender: mpsc::UnboundedSender<Command>,
}

impl ProjectRuntimeHandler {
    pub async fn subscribe_frame_stream(&self) -> FrameStream<(BackendFrame, Arc<RenderedFrame>)> {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::SubscribeFrameStream(tx))
            .expect("Project Runtime thread should not quit");
        let frame_stream = rx.await.expect("Project Runtime thread should not quit");
        frame_stream
    }
    pub async fn subscribe_scope(&self) -> FrameStream<ScopeGraph> {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::SubscribeScope(tx))
            .expect("Project Runtime thread should not quit");
        rx.await.expect("Project Runtime thread should not quit")
    }
    pub fn request_scope(&self, from: BackendFrame, width: usize, height: usize) {
        self.cmd_sender
            .unbounded_send(Command::RequestScope(from, width, height))
            .expect("Project Runtime thread should not quit")
    }
    pub async fn listen_on_media(&self) -> MediaSubscription {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::SubscribeMedia(tx))
            .expect("Project Runtime thread should not quit");
        let media_updater = rx.await.expect("Project Runtime thread should not quit");
        media_updater
    }
    pub async fn subscribe_thumbnails(&self) -> ThumbnailManagerClient {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::SubscribeThumbnailClient(tx))
            .expect("Project Runtime thread should not quit");
        rx.await.expect("Project Runtime thread should not quit")
    }
    pub async fn open_media(&self, path: &Path) -> SessionClient {
        let (tx, rx) = oneshot::channel();
        self.cmd_sender
            .unbounded_send(Command::OpenMedia {
                from: path.to_path_buf(),
                tx,
            })
            .expect("Project Runtime thread should not quit");
        rx.await.expect("Project Runtime thread should not quit")
    }
}

enum Command {
    // Subscribe commands: Bringing a Sender
    SubscribeFrameStream(oneshot::Sender<FrameStream<(BackendFrame, Arc<RenderedFrame>)>>),
    SubscribeScope(oneshot::Sender<FrameStream<ScopeGraph>>),
    SubscribeMedia(oneshot::Sender<MediaSubscription>),
    SubscribeThumbnailClient(oneshot::Sender<ThumbnailManagerClient>),
    // Requests
    RequestScope(BackendFrame, usize, usize),
    /// Open a new media.
    /// NOTE: This will discard old media
    OpenMedia {
        from: PathBuf,
        tx: oneshot::Sender<SessionClient>,
    },
}

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
use crate::project::session::Session;
use crate::render::Renderer;
use crate::render::{BackendFrame, FrameStream, RenderedFrame, ScopeGraph};
use crate::thumbnail::ThumbnailManager;
use crate::thumbnail::ThumbnailManagerClient;

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
                    _ = self.thumbnail_manager.handle_event().fuse() => {}
                }
            }
        });
    }

    fn re_render_current(&self) {
        if let Some(session) = &self.opened {
            self.renderer
                .render(session.gen_render_list(), session.get_decoded());
        }
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
            Command::SetBrightness(brightness) => {
                if let Some(session) = &mut self.opened {
                    session.set_brightness(brightness);
                }
                self.re_render_current();
            }
            Command::SetSaturation(saturation) => {
                if let Some(session) = &mut self.opened {
                    session.set_saturation(saturation);
                }
                self.re_render_current();
            }
            Command::OpenMedia(path) => {
                let Ok(editable) = Session::new(path.clone()) else {
                    error!("Failed to open media at {}", path.display());
                    return;
                };
                self.renderer
                    .render(editable.gen_render_list(), editable.get_decoded());
                self.opened = Some(editable);
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
            error!("ProjectRuntime thread panicked during shutdown!")
        }
    }
}

/// Communicate handler from the UI and the ProjectRuntime
#[derive(Clone)]
pub struct ProjectRuntimeHandler {
    pub(super) cmd_sender: mpsc::UnboundedSender<Command>,
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
    pub fn set_brightness(&self, brightness: f32) {
        self.cmd_sender
            .unbounded_send(Command::SetBrightness(brightness))
            .expect("Project Runtime thread should not quit")
    }

    pub fn set_saturation(&self, saturation: f32) {
        self.cmd_sender
            .unbounded_send(Command::SetSaturation(saturation))
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
    pub fn open_media(&self, path: &Path) {
        self.cmd_sender
            .unbounded_send(Command::OpenMedia(path.to_path_buf()))
            .expect("Project Runtime thread should not quit")
    }
}

pub enum Command {
    // Request commands: Bringing a Sender
    SubscribeFrameStream(oneshot::Sender<FrameStream<(BackendFrame, Arc<RenderedFrame>)>>),
    SubscribeScope(oneshot::Sender<FrameStream<ScopeGraph>>),
    RequestScope(BackendFrame, usize, usize),
    SubscribeMedia(oneshot::Sender<MediaSubscription>),
    SubscribeThumbnailClient(oneshot::Sender<ThumbnailManagerClient>),
    // Notify commands
    SetBrightness(f32),
    SetSaturation(f32),
    /// Open a new media.
    /// NOTE: This will discard old media
    OpenMedia(PathBuf),
}

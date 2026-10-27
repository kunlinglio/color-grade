use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use futures::FutureExt;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};
use rayon::ThreadPool;

use crate::media::manager::MediaManager;
use crate::project::edit_session::EditSession;
use crate::render::Renderer;
use crate::render::cpu::CPURenderer;
use crate::thumbnail::ThumbnailManager;

use super::comm::{Command, ProjectRuntimeHandler};

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    opened: Option<EditSession>,
    // Services
    renderer: Box<dyn Renderer>,
    media_manager: MediaManager,
    thumbnail_manager: ThumbnailManager,
}

impl ProjectRuntime {
    pub fn new(project_path: PathBuf, thread_pool: Arc<ThreadPool>) -> Self {
        let renderer: Box<dyn Renderer> = Box::new(CPURenderer::new(thread_pool.clone()));
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

    fn handle_command(&mut self, cmd: Command) {
        // Inside this method, we can ignore all the send/recv failure, since the runtime may lives longer than UI
        match cmd {
            Command::SubscribeFrameStream(tx) => {
                let frame_stream = self.renderer.subscribe();
                let _ = tx.send(frame_stream);
            }
            Command::SubscribeMedia(tx) => {
                let updater = self.media_manager.subscribe();
                let _ = tx.send(updater);
            }
            Command::ReqThumbnailClient(tx) => {
                let _ = tx.send(self.thumbnail_manager.client());
            }
            Command::WorkflowChanged => {
                if let Some(editable) = &self.opened {
                    self.renderer
                        .render(editable.gen_render_list(), editable.get_decoded());
                }
            }
            Command::OpenMedia(path) => {
                let Ok(editable) = EditSession::new(path.clone()) else {
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

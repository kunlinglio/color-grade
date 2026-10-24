use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use futures::FutureExt;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};
use rayon::ThreadPool;

use crate::media::manager::MediaManager;
use crate::project::communication::Command::{
    RequestMedias, SubscribeFrameStream, WorkflowChanged,
};
use crate::render_pipe::{CPURenderPipe, RenderPipe};

use super::communication::{Command, ProjectRuntimeHandler};

/// The super heavy struct, saving all the source needed for a opening project.
pub struct ProjectRuntime {
    project_path: PathBuf,
    renderer: Box<dyn RenderPipe>,
    thread_pool: Arc<ThreadPool>,
    media_manager: MediaManager,
}

impl ProjectRuntime {
    pub fn new(project_path: PathBuf, thread_pool: Arc<ThreadPool>) -> Self {
        let renderer: Box<dyn RenderPipe> =
            Box::new(CPURenderPipe::new(1920, 1080, thread_pool.clone()));
        let media_manager = MediaManager::new(project_path.clone(), thread_pool.clone());
        ProjectRuntime {
            project_path,
            renderer,
            thread_pool,
            media_manager,
        }
    }

    pub fn run(mut self) -> (ProjectRuntimeThread, ProjectRuntimeHandler) {
        let (quit_tx, quit_rx) = oneshot::channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        self.media_manager.begin_scan();
        let handler = thread::spawn(move || self.main_loop(quit_rx, cmd_rx));
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
                    _ = self.media_manager.handle_event().fuse() => {}
                }
            }
        });
    }

    fn handle_command(&mut self, cmd: Command) {
        // Inside this method, we can ignore all the send/recv failure, since the runtime may lives longer than UI
        match cmd {
            SubscribeFrameStream(tx) => {
                let frame_stream = self.renderer.subscribe();
                let _ = tx.send(frame_stream);
            }
            RequestMedias(tx) => {
                let updater = self.media_manager.subscribe();
                let _ = tx.send(updater);
            }
            WorkflowChanged => {
                self.renderer.submit_render();
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
        self.quit_sender.send(());
        self.handler.join();
    }
}

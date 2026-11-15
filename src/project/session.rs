use std::path::PathBuf;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::executor;

use crate::render::{Frame, Renderer};

use super::{WorkFlow, WorkFlowDiff};

/// A opened or edited media inside a project.
pub(super) struct Session {
    cmd_rx: mpsc::UnboundedReceiver<Command>,
    /// The ONLY SOURCE OF TRUTH of workflow
    workflow: WorkFlow,
    decoded: Arc<Frame>,
}

impl Session {
    pub fn new(path: PathBuf) -> anyhow::Result<(Self, SessionClient)> {
        let img = image::ImageReader::open(&path)?
            .with_guessed_format()?
            .decode()?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let decoded_frame = Arc::new(Frame::new(width as usize, height as usize, rgba.to_vec()));
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        let (notify_tx, notify_rx) = mpsc::unbounded();
        let workflow = WorkFlow::default();
        let session = Session {
            cmd_rx,
            workflow: workflow.clone(),
            decoded: decoded_frame,
        };
        let client = SessionClient {
            cmd_tx,
            workflow: Arc::new(workflow),
            notify_tx,
            notify_rx: Some(notify_rx),
        };
        Ok((session, client))
    }

    pub async fn handle_event(&mut self, renderer: &mut Renderer) {
        let Ok(cmd) = self.cmd_rx.recv().await else {
            return;
        };
        match cmd {
            Command::Update { diff, updater } => match self.workflow.apply_diff(diff) {
                Ok(need_render) => {
                    let snapshot = Arc::new(self.workflow.clone());
                    let _ = updater.send(Ok(snapshot));
                    if need_render {
                        self.render(renderer);
                    }
                }
                Err(e) => {
                    let _ = updater.send(Err(e));
                }
            },
        }
    }

    pub fn render(&self, renderer: &mut Renderer) {
        // TODO: Determine whether enable optimize by outer context
        match self.workflow.compile(true) {
            Ok(render_list) => {
                renderer.render(render_list, self.decoded.clone());
            }
            Err(e) => {
                warn!("Compile error: {e}, stop render it")
            }
        }
    }
}

pub struct SessionClient {
    cmd_tx: mpsc::UnboundedSender<Command>,
    /// A copy of workflow in Session.
    /// Although the workflow insixe Session is the only source of truth,
    /// all edit of workflow are based on client, so this copy is aligned with workflow
    /// in Session.
    workflow: Arc<WorkFlow>,
    notify_tx: mpsc::UnboundedSender<Arc<WorkFlow>>,
    /// Notify UI if workflow changed.
    /// This can only be taken once.
    notify_rx: Option<mpsc::UnboundedReceiver<Arc<WorkFlow>>>,
}

impl SessionClient {
    pub fn get_workflow(&self) -> Arc<WorkFlow> {
        self.workflow.clone()
    }

    pub fn update_workflow(&mut self, diff: WorkFlowDiff) -> bool {
        let (tx, rx) = oneshot::channel();
        let Ok(_) = self
            .cmd_tx
            .unbounded_send(Command::Update { diff, updater: tx })
        else {
            warn!("Failed to update workflow, because the Session has quit");
            return false;
        };
        let Ok(res) = executor::block_on(rx) else {
            warn!("Failed to update workflow, because the Session has quit");
            return false;
        };
        match res {
            Ok(new) => {
                self.workflow = new;
                let _ = self.notify_tx.unbounded_send(self.workflow.clone());
                true
            }
            Err(e) => {
                warn!("Failed to update workflow, because: {e:#}");
                false
            }
        }
    }

    pub fn subscribe_change(&mut self) -> mpsc::UnboundedReceiver<Arc<WorkFlow>> {
        self.notify_rx
            .take()
            .expect("Cannot subscribe change for SessionClient twice")
    }
}

enum Command {
    Update {
        diff: WorkFlowDiff,
        updater: oneshot::Sender<anyhow::Result<Arc<WorkFlow>>>,
    },
}

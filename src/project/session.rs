use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use futures::channel::mpsc;

use crate::render::{DecodedImage, RenderList};

use super::{Workflow, WorkflowDiff};

/// A opened or edited media inside a project.
pub(super) struct Session {
    cmd_rx: mpsc::UnboundedReceiver<Command>,
    /// The ONLY SOURCE OF TRUTH of workflow
    workflow: Workflow,
    decoded: Arc<DecodedImage>,
}

impl Session {
    pub fn new(path: PathBuf) -> anyhow::Result<(Self, SessionClient)> {
        let decoded = DecodedImage::decode_from(&path)
            .with_context(|| format!("Failed to create Session from {}", path.display()))?;
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        let workflow = Workflow::default();
        let session = Session {
            cmd_rx,
            workflow: workflow.clone(),
            decoded: Arc::new(decoded),
        };
        let client = SessionClient { cmd_tx, workflow };
        Ok((session, client))
    }

    pub async fn handle_event(&mut self, on_render: impl Fn(RenderList, Arc<DecodedImage>)) {
        let Ok(cmd) = self.cmd_rx.recv().await else {
            return;
        };
        match cmd {
            Command::Update { diff } => {
                let need_render = self
                    .workflow
                    .apply_diff(&diff)
                    .expect("The client has checked diff, here should not failed");
                if need_render {
                    self.render(on_render);
                }
            }
        }
    }
    pub fn render(&self, renderer: impl Fn(RenderList, Arc<DecodedImage>)) {
        // TODO: Determine whether enable optimize by outer context
        match self.workflow.compile(true) {
            Ok(render_list) => {
                renderer(render_list, self.decoded.clone());
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
    /// Although the workflow inside Session is the only source of truth,
    /// all edit of workflow are based on client, so this copy is aligned with workflow
    /// in Session.
    ///
    /// Since this copy is held by main thread, cached it can significantly increase
    /// reaction speed.
    workflow: Workflow,
}

impl SessionClient {
    pub fn get_workflow(&self) -> &Workflow {
        &self.workflow
    }

    pub fn update_workflow(&mut self, diff: WorkflowDiff) -> anyhow::Result<()> {
        self.workflow.verify_diff(&diff)?;
        let Ok(_) = self.cmd_tx.unbounded_send(Command::Update { diff }) else {
            anyhow::bail!("Failed to update workflow, because the Session has quit");
        };
        // Apply to workflow copy.
        // Since the copy is always synchronous with workflow inside Session,
        // so if this passed, the session will not return error, and we do not
        // need to wait for result comes from background threads.
        self.workflow
            .apply_diff(&diff)
            .expect("Verified diff should not be illegal");
        Ok(())
    }
}

enum Command {
    Update { diff: WorkflowDiff },
}

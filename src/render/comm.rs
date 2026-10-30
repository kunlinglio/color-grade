use std::sync::Arc;

use crate::render::RenderedFrame;

#[derive(Debug)]
pub struct FrameStream {
    rx: async_channel::Receiver<Arc<RenderedFrame>>,
}

impl FrameStream {
    pub async fn next_frame(&mut self) -> Option<Arc<RenderedFrame>> {
        self.rx.recv().await.ok()
    }
}

pub struct Publisher {
    senders: Vec<async_channel::Sender<Arc<RenderedFrame>>>,
    /// (request_version, frame)
    last_published: Option<(u64, Arc<RenderedFrame>)>,
}

impl Publisher {
    pub fn new() -> Self {
        Publisher {
            senders: Vec::new(),
            last_published: None,
        }
    }
    pub fn subscribe(&mut self) -> FrameStream {
        let (tx, rx) = async_channel::bounded(1);
        if let Some(frame) = self.last_published.clone() {
            let _ = tx.force_send(frame.1); // avoid have no frame inside the channel
        }
        self.senders.push(tx);
        FrameStream { rx }
    }
    pub fn publish(&mut self, current_gen: u64, frame: Arc<RenderedFrame>) {
        if let Some((request_gen, _)) = self.last_published {
            if current_gen <= request_gen {
                return;
            }
        }
        for tx in &self.senders {
            // Ignore the failure since the UI may quit before background tasks
            // TODO: Support unsubscribe if rx dropped
            let _ = tx.force_send(frame.clone());
        }
        self.last_published = Some((current_gen, frame));
    }
}

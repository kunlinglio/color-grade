use futures::channel::mpsc;

use super::media_type::Media;

pub enum Event {
    Sync(Vec<Media>),
    Add(Media),
}

#[derive(Debug)]
pub struct MediaUpdater {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl MediaUpdater {
    pub fn new(rx: mpsc::UnboundedReceiver<Event>) -> Self {
        MediaUpdater { rx }
    }
    pub async fn get(&mut self) -> Option<Event> {
        self.rx.recv().await.ok()
    }
}

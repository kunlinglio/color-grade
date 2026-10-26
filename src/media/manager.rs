use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::channel::mpsc;
use rayon::ThreadPool;

use crate::media::communication::{Event, MediaSubscription};

use super::media_type::Media;

pub struct MediaManager {
    root: PathBuf,
    thread_pool: Arc<ThreadPool>,

    /// Owned by rayon worker when scanning
    scan_tx: mpsc::UnboundedSender<Media>,
    scan_rx: mpsc::UnboundedReceiver<Media>,

    /// Path -> Media
    media: HashMap<PathBuf, Media>,
    /// Sender for ui subscriber
    /// TODO: Support remove subscribers
    ui_tx: Vec<mpsc::UnboundedSender<Event>>,
}

impl MediaManager {
    pub fn new(root: PathBuf, thread_pool: Arc<ThreadPool>) -> Self {
        let (scan_tx, scan_rx) = mpsc::unbounded::<Media>();
        MediaManager {
            root,
            thread_pool,
            scan_tx,
            scan_rx,
            media: HashMap::new(),
            ui_tx: Vec::new(),
        }
    }

    pub fn subscribe(&mut self) -> MediaSubscription {
        let (tx, rx) = mpsc::unbounded();
        let mut media_list = vec![];
        for v in self.media.values() {
            media_list.push(v.clone())
        }
        tx.unbounded_send(Event::Sync(media_list)).unwrap();
        self.ui_tx.push(tx);

        MediaSubscription::new(rx)
    }

    pub fn issue_scan(&self) {
        let root = self.root.clone();
        let tx = self.scan_tx.clone();
        self.thread_pool.spawn(move || {
            recursive_scan(&root, &mut |media| {
                // Ignore the failure since the UI may quit before background tasks
                let _ = tx.unbounded_send(media);
            });
        });
    }

    pub async fn handle_event(&mut self) {
        let new_media = self
            .scan_rx
            .recv()
            .await
            .expect("The channel should never destroy");
        for tx in &self.ui_tx {
            let _ = tx.unbounded_send(Event::Add(new_media.clone()));
        }
        self.media.insert(new_media.path.clone(), new_media);
    }
}

fn recursive_scan(from: impl AsRef<Path>, on_found: &mut impl FnMut(Media) -> ()) {
    let Ok(mut entries) = fs::read_dir(&from) else {
        warn!(
            "Failed to read directory entries from {}",
            from.as_ref().display()
        );
        return;
    };
    while let Some(entry) = entries.next() {
        let Ok(entry) = entry else {
            warn!(
                "Failed to read directory entry inside {}",
                from.as_ref().display()
            );
            continue;
        };
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            warn!("Failed to read metadata from {}", path.display());
            continue;
        };
        if metadata.is_dir() {
            recursive_scan(path, on_found);
        } else if metadata.is_file() {
            let Some(media) = Media::from_path(path) else {
                continue;
            };
            on_found(media)
        }
    }
}

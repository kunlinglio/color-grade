use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::channel::mpsc;
use rayon::ThreadPool;

use super::Media;

pub struct MediaManager {
    root: PathBuf,
    thread_pool: Arc<ThreadPool>,
    // medias: Vec<Media>,
}

impl MediaManager {
    pub fn new(root: PathBuf, thread_pool: Arc<ThreadPool>) -> Self {
        MediaManager { root, thread_pool }
    }

    pub fn begin_scan(&mut self) -> MediaUpdater {
        let root = self.root.clone();
        let thread_pool = Arc::clone(&self.thread_pool);

        let (mut tx, rx) = mpsc::channel(1024);

        thread_pool.spawn(move || {
            recursive_scan(&root, &mut |media| {
                tx.try_send(media).unwrap(); // FIXME: Remove this unwrap
            });
        });

        MediaUpdater { rx }
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
            return;
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

/// TODO: Support notify changes
#[derive(Debug)]
pub struct MediaUpdater {
    rx: mpsc::Receiver<Media>,
}

impl MediaUpdater {
    pub async fn get(&mut self) -> Option<Media> {
        self.rx.recv().await.ok()
    }
}

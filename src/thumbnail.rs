use std::collections::HashMap;
use std::num::NonZero;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use futures::channel::{mpsc, oneshot};
use image::GenericImageView;
use lru::LruCache;
use rayon::ThreadPool;

use crate::media::hash::MediaHash;
use crate::media::media_type::Media;
use crate::render::RenderedFrame;

const CACHE_CAPACITY: usize = 128;

/// The max size of thumbnail image.
/// (size = max(height, width))
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbnailSize {
    Px128,
    Px256,
    Px512,
    Px1024,
    Px2048,
}

impl ThumbnailSize {
    pub fn from_width(width: f32) -> Self {
        if width <= 128.0 {
            Self::Px128
        } else if width <= 256.0 {
            Self::Px256
        } else if width <= 512.0 {
            Self::Px512
        } else if width <= 1024.0 {
            Self::Px1024
        } else {
            Self::Px2048
        }
    }

    fn pixels(self) -> u32 {
        match self {
            Self::Px128 => 128,
            Self::Px256 => 256,
            Self::Px512 => 512,
            Self::Px1024 => 1024,
            Self::Px2048 => 2048,
        }
    }
}
pub struct Thumbnail {
    pub frame: RenderedFrame,
}

struct Cache {
    thumbnails: LruCache<(ThumbnailSize, MediaHash), Arc<Thumbnail>>,
    in_flight: HashMap<(ThumbnailSize, MediaHash), Vec<oneshot::Sender<Option<Arc<Thumbnail>>>>>,
}

impl Cache {
    fn new(capacity: NonZero<usize>) -> Self {
        Self {
            thumbnails: LruCache::new(capacity),
            in_flight: HashMap::new(),
        }
    }
}

pub struct ThumbnailManager {
    cache: Arc<Mutex<Cache>>,
    thread_pool: Arc<ThreadPool>,

    client_rx: mpsc::UnboundedReceiver<(
        Arc<Media>,
        ThumbnailSize,
        oneshot::Sender<Option<Arc<Thumbnail>>>,
    )>,
    /// Shared sender for all client
    client_tx: mpsc::UnboundedSender<(
        Arc<Media>,
        ThumbnailSize,
        oneshot::Sender<Option<Arc<Thumbnail>>>,
    )>,
}

impl ThumbnailManager {
    pub fn new(thread_pool: Arc<ThreadPool>) -> Self {
        let (client_tx, client_rx) = mpsc::unbounded();
        ThumbnailManager {
            cache: Arc::new(Mutex::new(Cache::new(
                NonZero::new(CACHE_CAPACITY).unwrap(),
            ))),
            thread_pool,
            client_rx,
            client_tx,
        }
    }

    pub fn client(&self) -> ThumbnailManagerClient {
        ThumbnailManagerClient {
            tx: self.client_tx.clone(),
        }
    }

    pub async fn handle_event(&mut self) {
        let Ok((media, size, tx)) = self.client_rx.recv().await else {
            return;
        };
        let key = (size, media.hash);
        {
            let mut cache_guard = self.cache.lock().unwrap();
            if let Some(thumb) = cache_guard.thumbnails.get(&key) {
                let _ = tx.send(Some(thumb.clone()));
                return;
            };
            if let Some(waiters) = cache_guard.in_flight.get_mut(&key) {
                waiters.push(tx);
                return;
            }
            cache_guard.in_flight.insert(key, vec![tx]);
        };
        let cache = self.cache.clone();
        self.thread_pool.spawn(move || {
            let res = load_thumbnail(&media.path, size);
            let mut cache_guard = cache.lock().unwrap();
            let waiters = cache_guard.in_flight.remove(&key).unwrap_or_default();
            let reply = match res {
                Ok(thumb) => {
                    let thumb = Arc::new(thumb);
                    cache_guard.thumbnails.put(key, thumb.clone());
                    Some(thumb)
                }
                Err(e) => {
                    warn!(
                        "Failed to load thumbnail for {}: {}",
                        media.path.display(),
                        e
                    );
                    None
                }
            };
            for waiter in waiters {
                let _ = waiter.send(reply.clone());
            }
        });
    }
}

/// A client object to handle binary communication between UI and thumbnail manager
#[derive(Clone)]
pub struct ThumbnailManagerClient {
    tx: mpsc::UnboundedSender<(
        Arc<Media>,
        ThumbnailSize,
        oneshot::Sender<Option<Arc<Thumbnail>>>,
    )>,
}

impl ThumbnailManagerClient {
    pub async fn request(&self, media: Arc<Media>, size: ThumbnailSize) -> Option<Arc<Thumbnail>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.unbounded_send((media, size, tx));
        rx.await
            .expect("The Project Runtime thread should not quit!")
    }
}

fn load_thumbnail(path: &PathBuf, size: ThumbnailSize) -> anyhow::Result<Thumbnail> {
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?;

    let (w, h) = img.dimensions();
    let max_size = size.pixels();

    let img = if w > max_size || h > max_size {
        img.thumbnail(max_size, max_size)
    } else {
        img
    };

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Thumbnail {
        frame: RenderedFrame::from_rgba(width as usize, height as usize, rgba.to_vec()),
    })
}

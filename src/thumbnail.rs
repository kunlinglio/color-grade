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

const THUMBNAIL_HEIGHT: usize = 256;
const CACHE_CAPACITY: usize = 256;

pub struct Thumbnail {
    pub frame: RenderedFrame,
}

struct Cache {
    thumbnails: LruCache<MediaHash, Arc<Thumbnail>>,
    in_flight: HashMap<MediaHash, Vec<oneshot::Sender<Option<Arc<Thumbnail>>>>>,
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

    client_rx: mpsc::UnboundedReceiver<(Arc<Media>, oneshot::Sender<Option<Arc<Thumbnail>>>)>,
    /// Shared sender for all client
    client_tx: mpsc::UnboundedSender<(Arc<Media>, oneshot::Sender<Option<Arc<Thumbnail>>>)>,
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
        let Ok((media, tx)) = self.client_rx.recv().await else {
            return;
        };
        {
            let mut cache_guard = self.cache.lock().unwrap();
            if let Some(thumb) = cache_guard.thumbnails.get(&media.hash) {
                let _ = tx.send(Some(thumb.clone()));
                return;
            };
            if let Some(waiters) = cache_guard.in_flight.get_mut(&media.hash) {
                waiters.push(tx);
                return;
            }
            cache_guard.in_flight.insert(media.hash, vec![tx]);
        };
        let cache = self.cache.clone();
        self.thread_pool.spawn(move || {
            let res = load_thumbnail(&media.path);
            let mut cache_guard = cache.lock().unwrap();
            let waiters = cache_guard
                .in_flight
                .remove(&media.hash)
                .unwrap_or_default();
            let reply = match res {
                Ok(thumb) => {
                    let thumb = Arc::new(thumb);
                    cache_guard.thumbnails.put(media.hash, thumb.clone());
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
    tx: mpsc::UnboundedSender<(Arc<Media>, oneshot::Sender<Option<Arc<Thumbnail>>>)>,
}

impl ThumbnailManagerClient {
    pub async fn request(&self, media: Arc<Media>) -> Option<Arc<Thumbnail>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.unbounded_send((media, tx));
        rx.await
            .expect("The Project Runtime thread should not quit!")
    }
}

fn load_thumbnail(path: &PathBuf) -> anyhow::Result<Thumbnail> {
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?;

    let (w, h) = img.dimensions();

    let img = if h > THUMBNAIL_HEIGHT as u32 {
        let new_w = ((w as f64 * THUMBNAIL_HEIGHT as f64) / h as f64).round() as u32;
        img.thumbnail(new_w.max(1), THUMBNAIL_HEIGHT as u32)
    } else {
        img
    };

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Thumbnail {
        frame: RenderedFrame::from_rgba(width as usize, height as usize, rgba.to_vec()),
    })
}

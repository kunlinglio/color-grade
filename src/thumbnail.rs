use std::num::NonZero;
use std::sync::{Arc, Mutex};

use futures::channel::{mpsc, oneshot};
use image::GenericImageView;
use lru::LruCache;
use rayon::ThreadPool;

use crate::media::hash::MediaHash;
use crate::media::media_type::Media;

const THUMBNAIL_HEIGHT: usize = 256;
const CACHE_CAPACITY: usize = 256;

pub struct Thumbnail {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

pub struct ThumbnailManager {
    cache: Arc<Mutex<LruCache<MediaHash, Arc<Thumbnail>>>>,
    thread_pool: Arc<ThreadPool>,

    client_rx: mpsc::UnboundedReceiver<(Media, oneshot::Sender<Option<Arc<Thumbnail>>>)>,
    /// Shared sender for all client
    client_tx: mpsc::UnboundedSender<(Media, oneshot::Sender<Option<Arc<Thumbnail>>>)>,
}

impl ThumbnailManager {
    pub fn new(thread_pool: Arc<ThreadPool>) -> Self {
        let (client_tx, client_rx) = mpsc::unbounded();
        ThumbnailManager {
            cache: Arc::new(Mutex::new(LruCache::new(
                NonZero::new(CACHE_CAPACITY).unwrap(),
            ))),
            thread_pool,
            client_rx,
            client_tx,
        }
    }

    pub fn subscribe(&self) -> ThumbnailManagerClient {
        ThumbnailManagerClient {
            tx: self.client_tx.clone(),
        }
    }

    pub async fn handle_event(&mut self) {
        let Ok(new_request) = self.client_rx.recv().await else {
            return;
        };
        let mut cache = self.cache.lock().unwrap();
        if let Some(thumb) = cache.get(&new_request.0.hash) {
            let _ = new_request.1.send(Some(thumb.clone()));
            return;
        };
        let cache = self.cache.clone();
        self.thread_pool
            .spawn(move || match load_thumbnail(&new_request.0) {
                Ok(thumb) => {
                    let thumb = Arc::new(thumb);
                    cache.lock().unwrap().put(new_request.0.hash, thumb.clone());
                    let _ = new_request.1.send(Some(thumb));
                }
                Err(e) => {
                    warn!(
                        "Failed to load thumbnail for {}: {}",
                        new_request.0.path.display(),
                        e
                    );
                    let _ = new_request.1.send(None);
                }
            });
    }
}

/// A client object to handle binary communication between UI and thumbnail manager
pub struct ThumbnailManagerClient {
    tx: mpsc::UnboundedSender<(Media, oneshot::Sender<Option<Arc<Thumbnail>>>)>,
}

impl ThumbnailManagerClient {
    pub async fn request(&self, media: Media) -> Option<Arc<Thumbnail>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.unbounded_send((media, tx));
        rx.await
            .expect("The Project Runtime thread should not quit!")
    }
}

fn load_thumbnail(media: &Media) -> anyhow::Result<Thumbnail> {
    let img = image::ImageReader::open(&media.path)?
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
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

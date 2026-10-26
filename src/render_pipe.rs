use std::sync::{Arc, Mutex};

use rayon::ThreadPool;

pub trait RenderPipe: Send {
    /// Notify RenderPipe to draw a new frame
    fn submit_render(&mut self);
    fn subscribe(&mut self) -> FrameStream;
}

const FILL_ODD: [u8; 4] = [128, 249, 134, 255];
const FILL_EVEN: [u8; 4] = [10, 28, 134, 255];

pub struct CPURenderPipe {
    width: usize,
    counter: usize,
    thread_pool: Arc<ThreadPool>,
    height: usize,
    publish_state: Arc<Mutex<PublishState>>,
}

struct PublishState {
    senders: Vec<async_channel::Sender<Arc<Frame>>>,
    version: u128,
}

impl CPURenderPipe {
    pub fn new(width: usize, height: usize, thread_pool: Arc<ThreadPool>) -> Self {
        let publish_state = Arc::new(Mutex::new(PublishState {
            senders: vec![],
            version: 0,
        }));
        Self {
            width,
            height,
            counter: 0,
            thread_pool,
            publish_state,
        }
    }
}

impl RenderPipe for CPURenderPipe {
    fn submit_render(&mut self) {
        self.counter += 1;

        let width = self.width;
        let height = self.height;
        let publish = self.publish_state.clone();
        let counter = self.counter;
        let current = publish.lock().unwrap().version + 1;
        self.thread_pool.spawn(move || {
            let latest = publish.lock().unwrap().version;
            if latest >= current {
                return;
            }
            let mut buffer = vec![0u8; width * height * 4];
            if counter % 2 == 0 {
                for pixel in buffer.chunks_exact_mut(4) {
                    pixel.copy_from_slice(&FILL_EVEN);
                }
            } else {
                for pixel in buffer.chunks_exact_mut(4) {
                    pixel.copy_from_slice(&FILL_ODD);
                }
            }
            let frame = Arc::new(Frame {
                width: width,
                height: height,
                rgba: buffer,
            });
            let mut publish_gurad = publish.lock().unwrap();
            let latest = publish_gurad.version;
            if latest >= current {
                return;
            }
            publish_gurad.version = current;
            for tx in &publish_gurad.senders {
                // Ignore the failure since the UI may quit before background tasks
                let _ = tx.force_send(frame.clone());
            }
        })
    }

    fn subscribe(&mut self) -> FrameStream {
        let (tx, rx) = async_channel::bounded(1);
        let mut publish_gurad = self.publish_state.lock().unwrap();
        publish_gurad.senders.push(tx);
        FrameStream { rx }
    }
}

#[derive(Debug)]
pub struct FrameStream {
    rx: async_channel::Receiver<Arc<Frame>>,
}

impl FrameStream {
    pub async fn next_frame(&mut self) -> Option<Arc<Frame>> {
        self.rx.recv().await.ok()
    }
}

#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

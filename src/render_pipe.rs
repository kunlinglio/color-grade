use std::sync::Arc;

use futures::StreamExt;
use futures::channel::mpsc;
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
    height: usize,
    senders: Vec<mpsc::UnboundedSender<Frame>>,
    counter: usize,
    thread_pool: Arc<ThreadPool>,
}

impl CPURenderPipe {
    pub fn new(width: usize, height: usize, thread_pool: Arc<ThreadPool>) -> Self {
        Self {
            width,
            height,
            senders: vec![],
            counter: 0,
            thread_pool,
        }
    }
}

impl RenderPipe for CPURenderPipe {
    fn submit_render(&mut self) {
        self.counter += 1;

        let width = self.width;
        let height = self.height;
        let senders = self.senders.clone();
        let counter = self.counter;
        self.thread_pool.spawn(move || {
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
            let frame = Frame {
                width: width,
                height: height,
                rgba: buffer,
            };
            for tx in &senders {
                // Ignore the failure since the UI may quit before background tasks
                let _ = tx.unbounded_send(frame.clone());
            }
        })
    }

    fn subscribe(&mut self) -> FrameStream {
        let (tx, rx) = mpsc::unbounded::<Frame>();
        self.senders.push(tx);
        FrameStream { rx }
    }
}

#[derive(Debug)]
pub struct FrameStream {
    /// TODO: This may cause memory leak, use some machenism to only product latest frame
    rx: mpsc::UnboundedReceiver<Frame>,
}

impl FrameStream {
    pub async fn next_frame(&mut self) -> Option<Frame> {
        self.rx.next().await
    }
}

#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

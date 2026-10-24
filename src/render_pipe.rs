use futures::StreamExt;
use futures::channel::mpsc;

pub trait RenderPipe {
    fn size(&self) -> (usize, usize);
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
}

impl CPURenderPipe {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            senders: vec![],
            counter: 0,
        }
    }
}

impl RenderPipe for CPURenderPipe {
    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn submit_render(&mut self) {
        self.counter += 1;
        let mut buffer = vec![0u8; self.width * self.height * 4];
        if self.counter % 2 == 0 {
            for pixel in buffer.chunks_exact_mut(4) {
                pixel.copy_from_slice(&FILL_EVEN);
            }
        } else {
            for pixel in buffer.chunks_exact_mut(4) {
                pixel.copy_from_slice(&FILL_ODD);
            }
        }
        let frame = Frame {
            width: self.width,
            height: self.height,
            rgba: buffer,
        };
        for tx in &self.senders {
            let _ = tx.unbounded_send(frame.clone());
        }
    }

    fn subscribe(&mut self) -> FrameStream {
        let (tx, rx) = mpsc::unbounded::<Frame>();
        self.senders.push(tx);
        FrameStream { rx }
    }
}

pub struct FrameStream {
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

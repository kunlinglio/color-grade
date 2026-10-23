use futures::channel::mpsc;
use futures::StreamExt;

pub trait RenderPipe {
    fn size(&self) -> (usize, usize);
    fn submit_render(&mut self);
    fn subscribe(&mut self) -> FrameStream;
}

pub struct CPURenderPipe {
    width: usize,
    height: usize,
    fill: [u8; 4], // RGBA
    senders: Vec<mpsc::UnboundedSender<Frame>>,
}

impl CPURenderPipe {
    pub fn new(width: usize, height: usize, fill: [u8; 4]) -> Self {
        Self {
            width,
            height,
            fill,
            senders: vec![],
        }
    }
}

impl RenderPipe for CPURenderPipe {
    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn submit_render(&mut self) {
        let mut buffer = vec![0u8; self.width * self.height * 4];
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.copy_from_slice(&self.fill);
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

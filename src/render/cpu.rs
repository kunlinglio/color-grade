use std::sync::{Arc, Mutex};

use rayon::ThreadPool;

use crate::render::render_list::Op;
use crate::render::render_list::RenderList;
use crate::render::{Frame, FrameStream, PublishState, RenderedFrame, Renderer};

pub struct CPURenderer {
    publish_state: Arc<Mutex<PublishState>>,
    thread_pool: Arc<ThreadPool>,
}

impl CPURenderer {
    pub fn new(thread_pool: Arc<ThreadPool>) -> Self {
        let publish_state = Arc::new(Mutex::new(PublishState {
            senders: vec![],
            version: 0,
        }));
        Self {
            thread_pool,
            publish_state,
        }
    }
    fn handle_render(list: RenderList, from: Arc<Frame>) -> Arc<RenderedFrame> {
        let mut buffer = IFrame::from_frame(&from);
        for op in list.ops {
            match op {
                Op::Brightness(b) => {
                    let factor = b.max(0.0);
                    for px in buffer.rgba.chunks_exact_mut(4) {
                        px[0] *= factor;
                        px[1] *= factor;
                        px[2] *= factor;
                    }
                }
                Op::Saturation(s) => {
                    for px in buffer.rgba.chunks_exact_mut(4) {
                        let y = 0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2];
                        px[0] = y + s * (px[0] - y);
                        px[1] = y + s * (px[1] - y);
                        px[2] = y + s * (px[2] - y);
                    }
                }
            }
        }
        Arc::new(RenderedFrame::from_frame(buffer.to_frame()))
    }
}

impl Renderer for CPURenderer {
    fn render(&self, list: RenderList, from: Arc<Frame>) {
        let publish = self.publish_state.clone();
        let current = publish.lock().unwrap().version + 1;
        self.thread_pool.spawn(move || {
            let latest = publish.lock().unwrap().version;
            if latest >= current {
                debug!("Renderer quit before computing, the task is outdate.");
                return;
            }
            let frame = CPURenderer::handle_render(list, from);
            let mut publish_gurad = publish.lock().unwrap();
            let latest = publish_gurad.version;
            if latest >= current {
                debug!("Renderer quit after computing, the task is outdate.");
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

/// Intermediate frame representation between process
struct IFrame {
    width: usize,
    height: usize,
    rgba: Vec<f32>,
}

impl IFrame {
    fn from_frame(frame: &Frame) -> Self {
        let mut rgba = Vec::new();
        rgba.reserve(frame.rgba.len());
        for e in &frame.rgba {
            rgba.push(e.clone() as f32);
        }
        IFrame {
            rgba,
            width: frame.width,
            height: frame.height,
        }
    }
    fn to_frame(&self) -> Frame {
        let mut rgba = Vec::new();
        rgba.reserve(self.rgba.len());
        for e in &self.rgba {
            rgba.push(e.clone() as u8);
        }
        Frame {
            width: self.width,
            height: self.height,
            rgba,
        }
    }
}

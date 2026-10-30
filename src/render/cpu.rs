use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rayon::ThreadPool;

use crate::render::comm::{FrameStream, Publisher};
use crate::render::render_list::Op;
use crate::render::render_list::RenderList;
use crate::render::{Frame, RenderedFrame, Renderer};

pub struct ScalarRenderer {
    publisher: Arc<Mutex<Publisher>>,
    /// This atomic variable is only for workers to check,
    /// does not influence the commit frame
    latest_request: Arc<AtomicU64>,
    thread_pool: Arc<ThreadPool>,
}

impl ScalarRenderer {
    pub fn new(thread_pool: Arc<ThreadPool>) -> Self {
        let publisher = Arc::new(Mutex::new(Publisher::new()));
        Self {
            thread_pool,
            latest_request: Arc::new(0.into()),
            publisher,
        }
    }
    fn handle_render(
        list: RenderList,
        from: Arc<Frame>,
        check_cancel: &impl Fn() -> bool,
    ) -> Option<Arc<RenderedFrame>> {
        let mut buffer = IFrame::from_frame(&from, &check_cancel)?;
        let width = from.width;
        for op in list.ops {
            if check_cancel() {
                return None;
            }
            match op {
                Op::Brightness(b) => {
                    let factor = b.max(0.0);
                    let row_stride = width * 4;
                    for row in buffer.rgba.chunks_exact_mut(row_stride) {
                        if check_cancel() {
                            return None;
                        }
                        for px in row.chunks_exact_mut(4) {
                            px[0] *= factor;
                            px[1] *= factor;
                            px[2] *= factor;
                        }
                    }
                }
                Op::Saturation(s) => {
                    let row_stride = width * 4;
                    for row in buffer.rgba.chunks_exact_mut(row_stride) {
                        if check_cancel() {
                            return None;
                        }
                        for px in row.chunks_exact_mut(4) {
                            let y = 0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2];
                            px[0] = y + s * (px[0] - y);
                            px[1] = y + s * (px[1] - y);
                            px[2] = y + s * (px[2] - y);
                        }
                    }
                }
            }
        }
        Some(Arc::new(RenderedFrame::from_frame(
            buffer.to_frame(&check_cancel)?,
        )))
    }
}

impl Renderer for ScalarRenderer {
    fn render(&self, list: RenderList, from: Arc<Frame>) {
        let publisher = self.publisher.clone();
        let latest = self.latest_request.clone();
        let current = latest.fetch_add(1, Ordering::Relaxed) + 1;
        let check_cancel = move || -> bool {
            let latest = latest.load(Ordering::Relaxed);
            latest != current
        };
        self.thread_pool.spawn(move || {
            if check_cancel() {
                return;
            }
            let Some(frame) = ScalarRenderer::handle_render(list, from, &check_cancel) else {
                // None indicates cancelled
                return;
            };
            if check_cancel() {
                return;
            }
            let publisher = publisher.lock();
            if let Ok(mut publisher) = publisher {
                publisher.publish(current, frame)
            }
        })
    }

    fn subscribe(&mut self) -> FrameStream {
        self.publisher.lock().unwrap().subscribe()
    }
}

/// Intermediate frame representation between process
struct IFrame {
    width: usize,
    height: usize,
    rgba: Vec<f32>,
}

impl IFrame {
    fn from_frame(frame: &Frame, check_cancel: &impl Fn() -> bool) -> Option<Self> {
        let mut rgba = Vec::new();
        rgba.reserve(frame.rgba.len());
        let row_stride = frame.width * 4;
        for row in frame.rgba.chunks_exact(row_stride) {
            if check_cancel() {
                return None;
            }
            for px in row {
                rgba.push(px.clone() as f32);
            }
        }
        Some(IFrame {
            rgba,
            width: frame.width,
            height: frame.height,
        })
    }
    fn to_frame(&self, check_cancel: &impl Fn() -> bool) -> Option<Frame> {
        let mut rgba = Vec::new();
        rgba.reserve(self.rgba.len());
        let row_stride = self.width * 4;
        for row in self.rgba.chunks_exact(row_stride) {
            if check_cancel() {
                return None;
            }
            for px in row {
                rgba.push(px.clone() as u8);
            }
        }
        Some(Frame {
            width: self.width,
            height: self.height,
            rgba,
        })
    }
}

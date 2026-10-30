use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rayon::ThreadPool;

use crate::render::comm::{FrameStream, Publisher};
use crate::render::render_list::Op;
use crate::render::render_list::RenderList;
use crate::render::{Frame, RenderedFrame, Renderer};

/// The most basic Renderer implementation.
/// Take no care about the performance, instead, focus on the correctness.
/// This will used in regression tests for highly-optimized renderer implementation.
pub struct ScalarRenderer {
    publisher: Arc<Mutex<Publisher>>,
    /// A counter to assign a unique generation id per render request.
    /// The publisher uses it to discard out-of-order frames.
    latest_request: Arc<AtomicU64>,
    /// State to make sure there is only single in flight task at any time.
    /// There are four state for `TaskState`:
    /// - (None, false): nothing to do, and no task is running
    /// - (Some, false): we have push one task into queue, but we haven't spawn worker (transient state)
    /// - (None, true): no more task waiting, but there is task running
    /// - (Some, true): we have a task waiting, and a task running
    state: Arc<Mutex<TaskState>>,
    thread_pool: Arc<ThreadPool>,
}

struct TaskState {
    /// The latest unrendered request (generation, render list, source frame).
    pending: Option<(u64, RenderList, Arc<Frame>)>,
    in_flight: bool,
}

impl ScalarRenderer {
    pub fn new(thread_pool: Arc<ThreadPool>) -> Self {
        Self {
            publisher: Arc::new(Mutex::new(Publisher::new())),
            latest_request: Arc::new(0.into()),
            state: Arc::new(Mutex::new(TaskState {
                pending: None,
                in_flight: false,
            })),
            thread_pool,
        }
    }

    fn handle_render(list: RenderList, from: Arc<Frame>) -> Arc<RenderedFrame> {
        let mut buffer = IFrame::from_frame(&from);
        let width = from.width;
        for op in list.ops {
            match op {
                Op::Brightness(b) => {
                    let factor = b.max(0.0);
                    let row_stride = width * 4;
                    for row in buffer.rgba.chunks_exact_mut(row_stride) {
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
        Arc::new(RenderedFrame::from_frame(buffer.to_frame()))
    }
}

impl Renderer for ScalarRenderer {
    fn render(&self, list: RenderList, from: Arc<Frame>) {
        let request_gen = self.latest_request.fetch_add(1, Ordering::Relaxed) + 1;
        let should_spawn = {
            let mut state = self.state.lock().unwrap();
            state.pending = Some((request_gen, list, from));
            if state.in_flight {
                false
            } else {
                state.in_flight = true;
                true
            }
        };
        if should_spawn {
            let state = self.state.clone();
            let publisher = self.publisher.clone();
            let thread_pool = self.thread_pool.clone();
            self.thread_pool
                .spawn(move || render_one(state, publisher, thread_pool));
        }
    }

    fn subscribe(&mut self) -> FrameStream {
        self.publisher.lock().unwrap().subscribe()
    }
}

/// To avoid take one thread from threadpool forever,
/// we spawn a new task after finish process a frame.
/// But it performs just like we have a thread running a infinite loop.
fn render_one(
    state: Arc<Mutex<TaskState>>,
    publisher: Arc<Mutex<Publisher>>,
    thread_pool: Arc<ThreadPool>,
) {
    let (request_gen, list, from) = {
        let mut state_guard = state.lock().unwrap();
        let job = state_guard.pending.take();
        let Some(req) = job else {
            // No more jobs, break this "loop"
            state_guard.in_flight = false;
            return;
        };
        req
    };
    let frame = ScalarRenderer::handle_render(list, from);
    {
        let mut publisher_guard = publisher.lock().unwrap();
        publisher_guard.publish(request_gen, frame);
    }
    thread_pool
        .clone()
        .spawn(move || render_one(state, publisher, thread_pool));
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

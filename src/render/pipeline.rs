use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rayon::ThreadPool;

#[derive(Debug)]
pub struct FrameStream<Res> {
    rx: async_channel::Receiver<Res>,
}

impl<Res> FrameStream<Res> {
    pub async fn next_frame(&mut self) -> Option<Res> {
        self.rx.recv().await.ok()
    }
}

struct Publisher<Res> {
    // TODO: Support clean subscribers
    senders: Vec<async_channel::Sender<Res>>,
    /// (request_version, frame)
    last_published: Option<(u64, Res)>,
}

impl<Res: Clone> Publisher<Res> {
    pub fn new() -> Self {
        Publisher {
            senders: Vec::new(),
            last_published: None,
        }
    }
    pub fn subscribe(&mut self) -> FrameStream<Res> {
        let (tx, rx) = async_channel::bounded(1);
        if let Some(frame) = self.last_published.clone() {
            let _ = tx.force_send(frame.1); // avoid have no frame inside the channel
        }
        self.senders.push(tx);
        FrameStream { rx }
    }
    pub fn publish(&mut self, current_gen: u64, frame: Res) {
        if let Some((request_gen, _)) = self.last_published {
            if current_gen <= request_gen {
                return;
            }
        }
        for tx in &self.senders {
            // Ignore the failure since the UI may quit before background tasks
            // TODO: Support unsubscribe if rx dropped
            let _ = tx.force_send(frame.clone());
        }
        self.last_published = Some((current_gen, frame));
    }
}

/// A generic single-flight, latest-wins compute pipeline.
/// Shared by multi component inside render/
pub struct Pipeline<Req, Res> {
    compute: Arc<dyn Fn(Req) -> Res + Send + Sync>,
    publisher: Arc<Mutex<Publisher<Res>>>,
    /// A counter to assign a unique generation id per render request.
    /// The publisher uses it to discard out-of-order frames.
    latest_request: Arc<AtomicU64>,
    /// State to make sure there is only single in flight task at any time.
    /// There are four state for `TaskState`:
    /// - (None, false): nothing to do, and no task is running
    /// - (Some, false): we have push one task into queue, but we haven't spawn worker (transient state)
    /// - (None, true): no more task waiting, but there is task running
    /// - (Some, true): we have a task waiting, and a task running
    state: Arc<Mutex<PipelineState<Req>>>,
    thread_pool: Arc<ThreadPool>,
}

struct PipelineState<Req> {
    /// The latest uncomputed request (generation, request).
    pending: Option<(u64, Req)>,
    in_flight: bool,
}

impl<Req, Res> Pipeline<Req, Res>
where
    Req: Send + 'static,
    Res: Clone + Send + Sync + 'static,
{
    pub fn new(
        thread_pool: Arc<ThreadPool>,
        compute: impl Fn(Req) -> Res + Send + Sync + 'static,
    ) -> Self {
        Self {
            compute: Arc::new(compute),
            publisher: Arc::new(Mutex::new(Publisher::new())),
            latest_request: Arc::new(0.into()),
            state: Arc::new(Mutex::new(PipelineState {
                pending: None,
                in_flight: false,
            })),
            thread_pool,
        }
    }

    /// Submit a request; the latest request wins.
    pub fn submit(&self, req: Req) {
        let request_gen = self.latest_request.fetch_add(1, Ordering::Relaxed) + 1;
        let should_spawn = {
            let mut state = self.state.lock().unwrap();
            state.pending = Some((request_gen, req));
            if state.in_flight {
                false
            } else {
                state.in_flight = true;
                true
            }
        };
        if should_spawn {
            let compute = self.compute.clone();
            let state = self.state.clone();
            let publisher = self.publisher.clone();
            let thread_pool = self.thread_pool.clone();
            self.thread_pool
                .spawn(move || Self::run_once(compute, state, publisher, thread_pool));
        }
    }

    pub fn subscribe(&mut self) -> FrameStream<Res> {
        self.publisher.lock().unwrap().subscribe()
    }

    /// Compute one pending request, then re-spawn a fresh task for the next one.
    /// This keeps the pipeline running without a long-lived loop, so no threadpool
    /// thread is held for the whole session.
    fn run_once(
        compute: Arc<dyn Fn(Req) -> Res + Send + Sync>,
        state: Arc<Mutex<PipelineState<Req>>>,
        publisher: Arc<Mutex<Publisher<Res>>>,
        thread_pool: Arc<ThreadPool>,
    ) {
        let (request_gen, req) = {
            let mut state_guard = state.lock().unwrap();
            let job = state_guard.pending.take();
            let Some(job) = job else {
                // No more jobs, break this "loop".
                state_guard.in_flight = false;
                return;
            };
            job
        };
        let frame = compute(req);
        {
            let mut publisher_guard = publisher.lock().unwrap();
            publisher_guard.publish(request_gen, frame);
        }
        thread_pool
            .clone()
            .spawn(move || Self::run_once(compute, state, publisher, thread_pool));
    }
}

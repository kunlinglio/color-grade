use std::any::Any;
use std::sync::Arc;

use rayon::ThreadPool;

use super::FrameStream;
use super::pipeline::Pipeline;
use super::render_list::RenderList;
use super::scalar::ScalarBackend;
use super::{Frame, RenderedFrame};

#[derive(Clone)]
pub struct ScopeGraph {
    pub waveform: Arc<RenderedFrame>,
    pub vectorscope: Arc<RenderedFrame>,
    pub y_histogram: Arc<RenderedFrame>,
    pub rgb_histogram: Arc<RenderedFrame>,
}

pub struct Renderer {
    /// Pipeline to handle frame render requests
    render_pipeline: Pipeline<(RenderList, Arc<Frame>), (BackendFrame, Arc<RenderedFrame>)>,
    /// Pipeline to handle scope render requests
    scope_pipeline: Pipeline<(BackendFrame, usize, usize), ScopeGraph>,
}

impl Renderer {
    pub fn new(thread_pool: Arc<ThreadPool>) -> Self {
        let render_backend = Arc::new(ScalarBackend::new());
        let scope_backend = render_backend.clone();
        let render_pipeline = Pipeline::new(thread_pool.clone(), move |(list, frame)| {
            render_backend.handle_render(list, frame)
        });
        let scope_pipeline = Pipeline::new(thread_pool, move |(from, width, height)| {
            scope_backend.handle_scope(from, width, height)
        });
        Renderer {
            render_pipeline,
            scope_pipeline,
        }
    }

    pub fn render(&self, list: RenderList, from: Arc<Frame>) {
        self.render_pipeline.submit((list, from));
    }

    pub fn subscribe(&mut self) -> FrameStream<(BackendFrame, Arc<RenderedFrame>)> {
        self.render_pipeline.subscribe()
    }

    /// Request once, return multiple scope graph.
    /// This may be a bit expensive, but can save response latency when user switch between different scope graph.
    /// And there may be some shared process between compute different graph, this can also be beneficial.
    pub fn request_scope(&self, from: BackendFrame, width: usize, height: usize) {
        self.scope_pipeline.submit((from, width, height));
    }

    pub fn subscribe_scope(&mut self) -> FrameStream<ScopeGraph> {
        self.scope_pipeline.subscribe()
    }
}

pub(super) trait RendererBackend: Send + Sync {
    fn handle_render(
        &self,
        list: RenderList,
        from: Arc<Frame>,
    ) -> (BackendFrame, Arc<RenderedFrame>);
    fn handle_scope(&self, from: BackendFrame, width: usize, height: usize) -> ScopeGraph;
}

/// An opaque pointer to a frame buffer from backend.
/// The user should not do any operation instead of manage it lifecycle or pass it to the Renderer.
#[derive(Clone)]
pub struct BackendFrame {
    inner: Arc<dyn Any + Send + Sync>,
}

impl BackendFrame {
    pub(crate) fn new<T: Any + Send + Sync>(frame: T) -> Self {
        Self {
            inner: Arc::new(frame),
        }
    }

    pub(crate) fn downcast<T: Any + Send + Sync>(&self) -> Arc<T> {
        self.inner
            .clone()
            .downcast::<T>()
            .expect("BackendFrame contains an unexpected frame type")
    }
}

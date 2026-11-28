use std::any::Any;
use std::sync::Arc;

use super::render_list::RenderList;
use super::{DecodedImage, RenderedFrame};

#[derive(Clone)]
pub struct ScopeGraph {
    pub waveform: Arc<RenderedFrame>,
    pub vectorscope: Arc<RenderedFrame>,
    pub y_histogram: Arc<RenderedFrame>,
    pub rgb_histogram: Arc<RenderedFrame>,
}

pub trait RenderBackend: Send + Sync {
    /// Render a full size picture for monitor using passed RenderList.
    fn handle_render(
        &self,
        list: RenderList,
        from: Arc<DecodedImage>,
    ) -> (BackendFrame, Arc<RenderedFrame>);

    /// Render all scopes from passed BackendFrame.
    fn handle_scope(&self, from: BackendFrame, width: usize, height: usize) -> ScopeGraph;
    /// Use color managed pipeline to render thumbnails.
    /// If you want to rendered in lower resolution, please sampled before calling this function.
    fn render_thumbnail(&self, from: Arc<DecodedImage>) -> Arc<RenderedFrame>;
}

/// An opaque pointer to a frame buffer from backend.
/// The user should not do any operation instead of manage it lifecycle or pass it to the backend.
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

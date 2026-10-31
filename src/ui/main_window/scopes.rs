use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::comm::ProjectRuntimeHandler;
use crate::render::{BackendFrame, FrameStream, RenderedFrame, ScopeGraph};

/// Fixed output size of the scope graphs (width = height).
const SCOPE_SIZE: usize = 256;

/// Scopes panel (waveform / vectorscope / histogram).
pub struct Scopes {
    waveform: Option<Arc<RenderImage>>,
    vectorscope: Option<Arc<RenderImage>>,
    y_histogram: Option<Arc<RenderImage>>,
    rgb_histogram: Option<Arc<RenderImage>>,
}

impl Scopes {
    pub fn new() -> Self {
        Scopes {
            waveform: None,
            vectorscope: None,
            y_histogram: None,
            rgb_histogram: None,
        }
    }

    pub fn init(
        &self,
        cx: &mut Context<Self>,
        window: &mut Window,
        mut runtime: ProjectRuntimeHandler,
    ) {
        let scope_runtime = runtime.clone();
        cx.spawn_in(window, async move |this, cx| {
            let frame_stream = runtime.subscribe_frame_stream().await;
            let scope_stream = runtime.subscribe_scope().await;
            let _ = this.update_in(cx, |scopes, window, cx| {
                scopes.start(window, cx, frame_stream, scope_stream, scope_runtime);
            });
        })
        .detach();
    }

    fn start(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mut frame_stream: FrameStream<(BackendFrame, Arc<RenderedFrame>)>,
        mut scope_stream: FrameStream<ScopeGraph>,
        scope_runtime: ProjectRuntimeHandler,
    ) {
        // Feed the scope pipeline with the latest backend frame.
        cx.spawn_in(window, async move |_this, _cx| {
            while let Some((backend_frame, _)) = frame_stream.next_frame().await {
                scope_runtime.request_scope(backend_frame, SCOPE_SIZE, SCOPE_SIZE);
            }
        })
        .detach();

        // Display the latest scope graph.
        cx.spawn_in(window, async move |this, cx| {
            while let Some(graph) = scope_stream.next_frame().await {
                let _ = this.update_in(cx, |scopes, _, cx| {
                    scopes.apply(graph);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn apply(&mut self, graph: ScopeGraph) {
        self.waveform = Some(to_render_image(&graph.waveform));
        self.vectorscope = Some(to_render_image(&graph.vectorscope));
        self.y_histogram = Some(to_render_image(&graph.y_histogram));
        self.rgb_histogram = Some(to_render_image(&graph.rgb_histogram));
    }
}

fn to_render_image(frame: &RenderedFrame) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(frame.to_image())]))
}

fn scope_item(label: &str, image: &Option<Arc<RenderImage>>) -> impl IntoElement {
    div()
        .v_flex()
        .flex_grow(1.0)
        .gap_1()
        .child(label.to_string())
        .child(match image {
            Some(i) => img(i.clone())
                .flex_grow(1.0)
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => div().flex_grow(1.0).into_any_element(),
        })
}

impl Render for Scopes {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(scope_item("Waveform", &self.waveform))
            .child(scope_item("Vectorscope", &self.vectorscope))
            .child(scope_item("Brightness Histogram", &self.y_histogram))
            .child(scope_item("Brightness Histogram", &self.rgb_histogram))
    }
}

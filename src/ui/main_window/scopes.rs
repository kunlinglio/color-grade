use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::comm::ProjectRuntimeHandler;
use crate::render::{BackendFrame, FrameStream, RenderedFrame, ScopeGraph};

pub struct Scopes {
    waveform: Option<Arc<RenderImage>>,
    vectorscope: Option<Arc<RenderImage>>,
    y_histogram: Option<Arc<RenderImage>>,
    rgb_histogram: Option<Arc<RenderImage>>,

    // Used to re-request after resize
    gram_size: Option<(usize, usize)>,
    last_frame: Option<BackendFrame>,

    runtime: ProjectRuntimeHandler,
}

impl Scopes {
    pub fn new(runtime: ProjectRuntimeHandler) -> Self {
        Scopes {
            waveform: None,
            vectorscope: None,
            y_histogram: None,
            rgb_histogram: None,
            gram_size: None,
            last_frame: None,
            runtime,
        }
    }

    pub fn init(&self, cx: &mut Context<Self>, window: &mut Window) {
        cx.spawn_in(window, async move |this, cx| {
            let Ok(runtime) = this.read_with(cx, |this, _cx| this.runtime.clone()) else {
                return;
            };
            let frame_stream = runtime.subscribe_frame_stream().await;
            let scope_stream = runtime.subscribe_scope().await;
            let _ = this.update_in(cx, |scopes, window, cx| {
                scopes.start(window, cx, frame_stream, scope_stream);
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
    ) {
        // Feed the scope pipeline with the latest backend frame.
        cx.spawn_in(window, async move |this, cx| {
            while let Some((backend_frame, _)) = frame_stream.next_frame().await {
                let _ = this.update(cx, move |this, _cx| {
                    this.request_update_gram(backend_frame);
                });
            }
        })
        .detach();

        // Display the latest scope graph.
        cx.spawn_in(window, async move |this, cx| {
            while let Some(graph) = scope_stream.next_frame().await {
                let _ = this.update(cx, |scopes, cx| {
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

    fn request_update_gram(&mut self, frame: BackendFrame) {
        self.last_frame = Some(frame.clone());
        let gram_size = self.gram_size;
        if let Some((w, h)) = gram_size {
            self.runtime.request_scope(frame, w, h);
        } else {
            warn!("Get a new frame, but the view hasn't prianted, dropped it!")
        }
    }

    fn scope_item(
        label: &str,
        image: &Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let print_label = label.to_string();
        let this = cx.weak_entity();
        div()
            .v_flex()
            .flex_1()
            .min_h_0()
            .gap_1()
            .child(label.to_string())
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .on_prepaint(move |bounds, _window, app| {
                        println!("{print_label}, bounds: {bounds}");
                        let _ = this.update(app, |this, _cx| {
                            let new_gram_size = Some((
                                usize::from(bounds.size.width.round()),
                                usize::from(bounds.size.height.round()),
                            ));
                            if new_gram_size != this.gram_size {
                                this.gram_size = new_gram_size;
                                if let Some(frame) = &this.last_frame {
                                    this.request_update_gram(frame.clone());
                                }
                            }
                        });
                    })
                    .child(match image {
                        Some(i) => img(i.clone())
                            .w_full()
                            .flex_1()
                            .min_h_0()
                            .object_fit(ObjectFit::Fill)
                            .into_any_element(),
                        None => div().flex_1().into_any_element(),
                    }),
            )
    }
}

fn to_render_image(frame: &RenderedFrame) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(frame.to_image())]))
}

impl Render for Scopes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(Self::scope_item("Waveform", &self.waveform, cx))
            .child(Self::scope_item("Vectorscope", &self.vectorscope, cx))
            .child(Self::scope_item(
                "Brightness Histogram",
                &self.y_histogram,
                cx,
            ))
            .child(Self::scope_item(
                "Brightness Histogram",
                &self.rgb_histogram,
                cx,
            ))
    }
}

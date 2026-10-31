use std::sync::Arc;

use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::comm::ProjectRuntimeHandler;
use crate::render::{BackendFrame, FrameStream, RenderedFrame, ScopeGraph};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChosenScope {
    BrightnessHistogram,
    RGBHistogram,
    Vectorscope,
    Waveform,
}

impl ChosenScope {
    fn short_label(&self) -> &'static str {
        match self {
            Self::BrightnessHistogram => "His",
            Self::RGBHistogram => "RGBHis",
            Self::Vectorscope => "Vec",
            Self::Waveform => "WF",
        }
    }

    fn element_id(&self) -> &'static str {
        match self {
            Self::BrightnessHistogram => "scope-brightness-histogram",
            Self::RGBHistogram => "scope-rgb-histogram",
            Self::Vectorscope => "scope-vectorscope",
            Self::Waveform => "scope-waveform",
        }
    }
}

pub struct Scopes {
    waveform: Option<Arc<RenderImage>>,
    vectorscope: Option<Arc<RenderImage>>,
    y_histogram: Option<Arc<RenderImage>>,
    rgb_histogram: Option<Arc<RenderImage>>,

    // Used to re-request after resize
    gram_size: Option<(usize, usize)>,
    last_frame: Option<BackendFrame>,

    runtime: ProjectRuntimeHandler,

    chosen: ChosenScope,
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
            chosen: ChosenScope::Waveform,
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
                let _ = this.update_in(cx, |scopes, window, cx| {
                    scopes.apply(graph, window);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn apply(&mut self, graph: ScopeGraph, window: &mut Window) {
        if let Some(previous) = self.waveform.replace(to_render_image(&graph.waveform)) {
            let _ = window.drop_image(previous);
        };
        if let Some(previous) = self
            .vectorscope
            .replace(to_render_image(&graph.vectorscope))
        {
            let _ = window.drop_image(previous);
        };
        if let Some(previous) = self
            .y_histogram
            .replace(to_render_image(&graph.y_histogram))
        {
            let _ = window.drop_image(previous);
        };
        if let Some(previous) = self
            .rgb_histogram
            .replace(to_render_image(&graph.rgb_histogram))
        {
            let _ = window.drop_image(previous);
        };
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

    fn get_image_by_chosen(&self) -> &Option<Arc<RenderImage>> {
        match self.chosen {
            ChosenScope::BrightnessHistogram => &self.y_histogram,
            ChosenScope::RGBHistogram => &self.rgb_histogram,
            ChosenScope::Vectorscope => &self.vectorscope,
            ChosenScope::Waveform => &self.waveform,
        }
    }
}

fn to_render_image(frame: &RenderedFrame) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(frame.to_image())]))
}

fn scope_button(
    this: WeakEntity<Scopes>,
    scope: ChosenScope,
    chosen: ChosenScope,
) -> impl IntoElement {
    let selected = chosen == scope;
    Button::new(scope.element_id())
        .label(scope.short_label())
        .ghost()
        .compact()
        .selected(selected)
        .on_click(move |_, _window, app| {
            let _ = this.update(app, |scopes, cx| {
                scopes.chosen = scope;
                cx.notify();
            });
        })
}

impl Render for Scopes {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        div()
            .v_flex()
            .size_full()
            .p_2()
            .gap_1()
            .child(
                div()
                    .flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(text!("Scopes"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(scope_button(
                                this.clone(),
                                ChosenScope::Waveform,
                                self.chosen,
                            ))
                            .child(scope_button(
                                this.clone(),
                                ChosenScope::Vectorscope,
                                self.chosen,
                            ))
                            .child(scope_button(
                                this.clone(),
                                ChosenScope::BrightnessHistogram,
                                self.chosen,
                            ))
                            .child(scope_button(
                                this.clone(),
                                ChosenScope::RGBHistogram,
                                self.chosen,
                            )),
                    ),
            )
            .child(
                div().v_flex().flex_1().min_h_0().child(
                    div()
                        .v_flex()
                        .w_full()
                        .flex_1()
                        .min_h_0()
                        .on_prepaint(move |bounds, _window, app| {
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
                        .child(match self.get_image_by_chosen() {
                            Some(i) => img(i.clone())
                                .w_full()
                                .flex_1()
                                .min_h_0()
                                .object_fit(ObjectFit::Fill)
                                .into_any_element(),
                            None => div().flex_1().into_any_element(),
                        }),
                ),
            )
    }
}

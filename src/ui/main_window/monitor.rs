use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::comm::ProjectRuntimeHandler;
use crate::render::{BackendFrame, FrameStream, RenderedFrame};

pub struct Monitor {
    image: Option<Arc<RenderImage>>,
}

impl Monitor {
    pub fn new() -> Self {
        Monitor { image: None }
    }

    pub fn init(
        &self,
        cx: &mut Context<Self>,
        window: &mut Window,
        mut runtime: ProjectRuntimeHandler,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            // Subscribe to the render stream.
            let frame_stream = runtime.subscribe_frame_stream().await;
            let _ = this.update_in(cx, |monitor, window, cx| {
                monitor.start(window, cx, frame_stream);
            });
        })
        .detach();
    }

    /// Start to monitor the result from RenderPipe
    fn start(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mut frame_stream: FrameStream<(BackendFrame, Arc<RenderedFrame>)>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some((_, frame)) = frame_stream.next_frame().await {
                let _ = this.update_in(cx, |monitor, window, cx| {
                    monitor.apply_buffer(window, frame);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn apply_buffer(&mut self, window: &mut Window, frame: Arc<RenderedFrame>) {
        let new_image = Arc::new(RenderImage::new(vec![image::Frame::new(frame.to_image())]));
        if let Some(previous) = self.image.replace(new_image) {
            // `RenderImage::new` assigns a fresh id every frame, and `paint_image`
            // uploads it to the window's sprite atlas, which never evicts tiles.
            // Drop the previous image's tile or every rendered frame leaks GPU memory.
            let _ = window.drop_image(previous);
        }
    }
}

impl Render for Monitor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::div()
            .v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(match &self.image {
                Some(i) => img(i.clone())
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => "No Frame applied".into_any_element(),
            })
    }
}

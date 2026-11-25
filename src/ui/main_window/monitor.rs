use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::ProjectRuntimeHandler;
use crate::render::RenderedFrame;
use crate::ui::components::{Icon, ResizeSensitive, Text};
use crate::ui::get_theme;

#[derive(Clone)]
struct MonitorDrag;

pub struct Monitor {
    image: Option<Arc<RenderImage>>,
    _listen_task: Task<()>,
    // States for moving and zooming
    image_size: Option<(usize, usize)>,
    viewport: Option<Bounds<Pixels>>,
    zoom_fit: f32,
    zoom: f32,
    /// View center -> Frame center
    pan: Point<Pixels>,
    last_cursor: Option<Point<Pixels>>,
}

impl Monitor {
    pub fn new(cx: &mut Context<Self>, runtime: ProjectRuntimeHandler) -> Self {
        let _listen_task = cx.spawn(async move |this, cx| {
            // Subscribe to the render stream.
            let mut frame_stream = runtime.subscribe_frame_stream().await;
            while let Some((_, frame)) = frame_stream.next_frame().await {
                let _ = this.update_in(cx, |monitor, window, cx| {
                    monitor.apply_buffer(window, frame);
                    cx.notify();
                });
            }
        });
        Monitor {
            image: None,
            image_size: None,
            _listen_task,
            viewport: None,
            zoom_fit: 1.0,
            zoom: 1.0,
            pan: Point::default(),
            last_cursor: None,
        }
    }

    fn apply_buffer(&mut self, window: &mut Window, frame: Arc<RenderedFrame>) {
        let new_image = Arc::new(RenderImage::new(vec![image::Frame::new(frame.to_image())]));
        if let Some(previous) = self.image.replace(new_image) {
            // `RenderImage::new` assigns a fresh id every frame, and `paint_image`
            // uploads it to the window's sprite atlas, which never evicts tiles.
            // Drop the previous image's tile or every rendered frame leaks GPU memory.
            let _ = window.drop_image(previous);
        }
        // TODO: Support re-fit when session changed, rather than resolution changed
        if self.image_size != Some(frame.size()) {
            self.image_size = Some(frame.size());
            self.last_cursor = None;
            self.fit();
        }
    }

    fn drag_move(&mut self, event: &DragMoveEvent<MonitorDrag>, cx: &mut Context<Self>) {
        let cur_cursor = event.event.position;
        let Some(last_cursor) = &mut self.last_cursor else {
            self.last_cursor = Some(cur_cursor);
            return;
        };
        self.pan += cur_cursor - *last_cursor;
        *last_cursor = cur_cursor;
        cx.notify();
    }

    fn drag_finish(&mut self) {
        self.last_cursor = None;
    }

    fn zoom_at(&mut self, cursor: Point<Pixels>, factor: f32) {
        if self.zoom <= 0. {
            return;
        }
        let Some(viewport) = &self.viewport else {
            warn!("Zoom Monitor before prepaint, ignore it");
            return;
        };
        // scroll point in view coordinate
        let scroll_point = point(
            cursor.x - viewport.origin.x - viewport.size.width / 2.,
            cursor.y - viewport.origin.y - viewport.size.height / 2.,
        );

        let new_zoom = (self.zoom * factor).clamp(0.5, 10.0);
        let ratio = new_zoom / self.zoom;

        self.pan = point(
            scroll_point.x - (scroll_point.x - self.pan.x) * ratio,
            scroll_point.y - (scroll_point.y - self.pan.y) * ratio,
        );
        self.zoom = new_zoom;
    }

    fn fit(&mut self) {
        let Some((w, h)) = self.image_size else {
            warn!("Cannot fit view without frame loaded");
            return;
        };
        if let Some(viewport) = &self.viewport {
            self.zoom_fit = (f32::from(viewport.size.width) / w as f32)
                .min(f32::from(viewport.size.height) / h as f32)
        }
        self.zoom = 1.0;
        self.pan = Point::default();
    }
}

impl Render for Monitor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let this = cx.weak_entity();
        ResizeSensitive::new()
            .size_full()
            // FIXME: This could cause incorrect bounds
            .pb_1()
            .overflow_hidden()
            .on_resize(move |bounds, _window, app| {
                let _ = this.update(app, |this, cx| {
                    if this.viewport != Some(bounds) {
                        this.viewport = Some(bounds);
                        cx.notify();
                    }
                });
            })
            .child(match &self.image {
                Some(i) => div()
                    .id("monitor-viewport")
                    .relative()
                    .size_full()
                    // we do not need any drag ghost, so construct a empty entity
                    .on_drag(MonitorDrag, |_id, _point, _window, cx| cx.new(|_| Empty))
                    .on_drag_move(cx.listener(
                        |this, event: &DragMoveEvent<MonitorDrag>, _window, cx| {
                            this.drag_move(event, cx);
                        },
                    ))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, _cx| {
                            this.drag_finish();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, _cx| this.drag_finish()),
                    )
                    .on_scroll(cx.listener(|this, event: &ScrollEvent, _window, cx| {
                        match event.delta {
                            ScrollDelta::Lines(delta) => {
                                // wheel scrolling: zoom
                                this.zoom_at(event.position, (delta.y * 0.15).exp());
                            }
                            ScrollDelta::Pixels(delta) => {
                                // touch pad scroll
                                if event.modifiers.secondary(cx) {
                                    // CMD/Ctrl + scroll: zooms
                                    let dy = f32::from(delta.y);
                                    this.zoom_at(event.position, (dy * 0.003).exp());
                                } else {
                                    // move
                                    this.pan += delta;
                                }
                            }
                        }
                        cx.notify();
                    }))
                    .on_pinch(cx.listener(|this, event: &PinchEvent, _window, cx| {
                        this.zoom_at(event.position, 1.0 + event.delta);
                        cx.notify();
                    }))
                    .child({
                        // frame width, height in view
                        let (w, h) = (
                            px(self.image_size.unwrap().0 as f32 * self.zoom * self.zoom_fit),
                            px(self.image_size.unwrap().1 as f32 * self.zoom * self.zoom_fit),
                        );
                        img(i.clone())
                            .left(self.viewport.unwrap().size.width / 2. - w / 2. + self.pan.x)
                            .top(self.viewport.unwrap().size.height / 2. - h / 2. + self.pan.y)
                            .w(w)
                            .h(h)
                    })
                    .into_any_element(),
                None => div()
                    .size_full()
                    .v_flex()
                    .flex_1()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        Icon::new(assets::IconName::ImagePlus)
                            .text_color(theme.colors.text.secondary)
                            .size(px(50.)),
                    )
                    .child(Text::new("Open a media to start editing.").content())
                    .into_any_element(),
            })
    }
}

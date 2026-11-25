use std::sync::Arc;

use anyhow::Context as _;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::ProjectRuntimeHandler;
use crate::render::RenderedFrame;
use crate::ui::components::{Icon, Loading, ResizeSensitive, Text};
use crate::ui::get_theme;

use super::state::{self, EditState};

#[derive(Clone)]
struct MonitorDrag;

pub struct Monitor {
    viewport: Option<Bounds<Pixels>>,
    frame_state: FrameState,
    _listen_frame_task: Task<()>,
    _listen_session_change_sub: Subscription,
}

enum FrameState {
    NoSession,
    Loading,
    Ready {
        frame: Arc<RenderImage>,
        frame_size: (usize, usize),

        // States for moving and zooming
        /// The relative zoom to the piecutre.
        /// Real zoon = fit_scale * zoom
        zoom: f32,
        /// View center -> Frame center
        pan: Point<Pixels>,
        last_cursor: Option<Point<Pixels>>,
    },
}

impl FrameState {
    fn new_ready(frame: Arc<RenderImage>, frame_size: (usize, usize)) -> Self {
        Self::Ready {
            frame,
            frame_size,
            zoom: 1.0,
            pan: Point::default(),
            last_cursor: None,
        }
    }
}

impl Monitor {
    pub fn new(
        cx: &mut Context<Self>,
        runtime: ProjectRuntimeHandler,
        edit_state: Entity<EditState>,
        window: &mut Window,
    ) -> Self {
        let _listen_frame_task = cx.spawn(async move |this, cx| {
            // Subscribe to the render stream.
            let mut frame_stream = runtime.subscribe_frame_stream().await;
            while let Some((_, frame)) = frame_stream.next_frame().await {
                log_if_err!(
                    this.update_in(cx, |monitor, window, cx| {
                        monitor.apply_buffer(window, frame);
                        cx.notify();
                    })
                    .context("Failed to apply rendered frame to Monitor")
                );
            }
        });
        let _listen_session_change_sub = cx.subscribe_in(
            &edit_state,
            window,
            |this, _edit_state, evt: &state::SessionChanged, window, cx| {
                let old_state = match evt.new {
                    Some(_) => std::mem::replace(&mut this.frame_state, FrameState::Loading),
                    None => std::mem::replace(&mut this.frame_state, FrameState::NoSession),
                };
                if let FrameState::Ready { frame, .. } = old_state {
                    log_if_err!(
                        window
                            .drop_image(frame)
                            .context("Failed to drop old frame texture in Monitor")
                    )
                }
                cx.notify()
            },
        );
        Monitor {
            viewport: None,
            frame_state: FrameState::NoSession,
            _listen_session_change_sub,
            _listen_frame_task,
        }
    }

    fn apply_buffer(&mut self, window: &mut Window, new_frame: Arc<RenderedFrame>) {
        let size = new_frame.size();
        let new_frame = Arc::new(RenderImage::new(vec![image::Frame::new(
            new_frame.to_image(),
        )]));
        if let FrameState::Ready {
            frame, frame_size, ..
        } = &mut self.frame_state
        {
            let previous = std::mem::replace(frame, new_frame);
            // `RenderImage::new` assigns a fresh id every frame, and `paint_image`
            // uploads it to the window's sprite atlas, which never evicts tiles.
            // Drop the previous image's tile or every rendered frame leaks GPU memory.
            log_if_err!(
                window
                    .drop_image(previous)
                    .context("Failed to drop old frame texture in Monitor")
            );
            *frame_size = size;
        } else {
            self.frame_state = FrameState::new_ready(new_frame, size);
        }
    }

    fn drag_move(&mut self, event: &DragMoveEvent<MonitorDrag>, cx: &mut Context<Self>) {
        let FrameState::Ready {
            pan, last_cursor, ..
        } = &mut self.frame_state
        else {
            return;
        };
        let cur_cursor = event.event.position;
        let Some(last_cursor) = last_cursor else {
            *last_cursor = Some(cur_cursor);
            return;
        };
        *pan += cur_cursor - *last_cursor;
        *last_cursor = cur_cursor;
        cx.notify();
    }

    fn drag_finish(&mut self) {
        let FrameState::Ready { last_cursor, .. } = &mut self.frame_state else {
            return;
        };
        *last_cursor = None;
    }

    fn zoom_at(&mut self, cursor: Point<Pixels>, factor: f32) {
        let FrameState::Ready { zoom, pan, .. } = &mut self.frame_state else {
            return;
        };
        if *zoom <= 0. {
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

        let new_zoom = (*zoom * factor).clamp(0.5, 10.0);
        let ratio = new_zoom / *zoom;

        *pan = point(
            scroll_point.x - (scroll_point.x - pan.x) * ratio,
            scroll_point.y - (scroll_point.y - pan.y) * ratio,
        );
        *zoom = new_zoom;
    }

    fn fit_scale(&self) -> Option<f32> {
        let FrameState::Ready { frame_size, .. } = &self.frame_state else {
            return None;
        };
        let (w, h) = *frame_size;
        let viewport = &self.viewport?;
        Some(
            (f32::from(viewport.size.width) / w as f32)
                .min(f32::from(viewport.size.height) / h as f32),
        )
    }
}

impl Render for Monitor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let this = cx.weak_entity();
        div().size_full().pb_1().overflow_hidden().child(
            ResizeSensitive::new()
                .size_full()
                .on_resize(move |bounds, _window, app| {
                    log_if_err!(
                        this.update(app, |this, cx| {
                            if this.viewport != Some(bounds) {
                                this.viewport = Some(bounds);
                                cx.notify();
                            }
                        })
                        .context("Failed to update Monitor viewport size")
                    );
                })
                .child(match &self.frame_state {
                    FrameState::NoSession => div()
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
                    FrameState::Loading => div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Loading::new().size(px(50.)))
                        .into_any_element(),
                    FrameState::Ready {
                        frame,
                        frame_size,
                        zoom,
                        pan,
                        ..
                    } => {
                        div()
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
                                            if let FrameState::Ready { pan, .. } =
                                                &mut this.frame_state
                                            {
                                                *pan += delta;
                                            }
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
                                let fit_scale = self.fit_scale().unwrap_or(1.0);
                                // frame width, height in view
                                let (w, h) = (
                                    px(frame_size.0 as f32 * *zoom * fit_scale),
                                    px(frame_size.1 as f32 * *zoom * fit_scale),
                                );
                                if let Some(viewport) = self.viewport {
                                    img(frame.clone())
                                        .left(viewport.size.width / 2. - w / 2. + pan.x)
                                        .top(viewport.size.height / 2. - h / 2. + pan.y)
                                        .w(w)
                                        .h(h)
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                }
                            })
                            .into_any_element()
                    }
                }),
        )
    }
}

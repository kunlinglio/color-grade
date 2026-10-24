use gpui_kit::component::*;
use gpui_kit::*;

use crate::media;
use crate::media::communication::MediaUpdater;
use crate::media::media_type::Media;
use crate::project::communication::ProjectRuntimeHandler;

pub struct Library {
    media_list: Vec<Media>,
}

impl Library {
    pub fn new() -> Self {
        Library { media_list: vec![] }
    }

    pub fn init(
        &self,
        cx: &mut Context<Self>,
        window: &mut Window,
        mut runtime: ProjectRuntimeHandler,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            // Begin scanning media.
            let media_rx = runtime.request_media().await;
            let _ = this.update_in(cx, |library, window, cx| {
                library.start(window, cx, media_rx);
            });
        })
        .detach();
    }

    /// Start to monitor the result from RenderPipe
    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>, mut rx: MediaUpdater) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some(e) = rx.get().await {
                let _ = cx.update(|_window, app| {
                    let _ = this.update(app, |this, cx| {
                        match e {
                            media::communication::Event::Sync(media_list) => {
                                this.media_list = media_list;
                            }
                            media::communication::Event::Add(media) => {
                                this.media_list.push(media);
                            }
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }
}

impl Render for Library {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let medias = self.media_list.clone();
        div()
            .v_flex()
            .size_full()
            .child(text!("Library View"))
            .child(
                uniform_list("library", medias.len(), move |range, _window, _cx| {
                    range
                        .map(|i| {
                            div()
                                .h(px(24.0))
                                .child(medias[i].path.to_string_lossy().into_owned())
                        })
                        .collect()
                })
                .flex_grow(1.0)
                .into_any_element(),
            )
    }
}

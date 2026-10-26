use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::media;
use crate::media::communication::MediaSubscription;
use crate::media::hash::MediaHash;
use crate::media::media_type::Media;
use crate::project::communication::ProjectRuntimeHandler;
use crate::thumbnail::ThumbnailManagerClient;

pub struct Library {
    media_list: Vec<Media>,
    thumbnails: HashMap<MediaHash, Arc<RenderImage>>,
}

impl Library {
    pub fn new() -> Self {
        Library {
            media_list: vec![],
            thumbnails: HashMap::new(),
        }
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
            let thumbnail_client = runtime.subscribe_thumbnails().await;
            let _ = this.update_in(cx, |library, window, cx| {
                library.start(window, cx, media_rx, thumbnail_client);
            });
        })
        .detach();
    }

    /// Monitor media events and fetch thumbnails in the background.
    fn start(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mut rx: MediaSubscription,
        thumbnail_client: ThumbnailManagerClient,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some(e) = rx.get().await {
                match e {
                    media::communication::Event::Sync(list) => {
                        for media in &list {
                            spawn_thumbnail(
                                this.clone(),
                                cx.clone(),
                                thumbnail_client.clone(),
                                media.clone(),
                            );
                        }
                        let _ = cx.update(|_, app| {
                            let _ = this.update(app, |library, cx| {
                                library.media_list = list;
                                cx.notify();
                            });
                        });
                    }
                    media::communication::Event::Add(media) => {
                        spawn_thumbnail(
                            this.clone(),
                            cx.clone(),
                            thumbnail_client.clone(),
                            media.clone(),
                        );
                        let _ = cx.update(|_, app| {
                            let _ = this.update(app, |library, cx| {
                                library.media_list.push(media);
                                cx.notify();
                            });
                        });
                    }
                }
            }
        })
        .detach();
    }
}

/// Fetch one thumbnail in the background, then cache it as a `RenderImage`.
fn spawn_thumbnail(
    this: WeakEntity<Library>,
    cx: AsyncWindowContext,
    client: ThumbnailManagerClient,
    media: Media,
) {
    cx.spawn(async move |cx| {
        if let Some(thumb) = client.request(media.clone()).await {
            let image = Arc::new(RenderImage::new(vec![image::Frame::new(
                thumb.frame.to_image(),
            )]));
            let hash = media.hash;
            let _ = cx.update(|_, app| {
                let _ = this.update(app, |library, cx| {
                    library.thumbnails.insert(hash, image);
                    cx.notify();
                });
            });
        }
    })
    .detach();
}

impl Render for Library {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let medias = self.media_list.clone();
        let thumbnails = self.thumbnails.clone();
        div()
            .v_flex()
            .size_full()
            .child(text!("Library View"))
            .child(
                uniform_list("library_list", medias.len(), move |range, _window, _cx| {
                    range
                        .map(|i| {
                            let media = &medias[i];
                            let thumb = thumbnails.get(&media.hash).cloned();
                            let file_name = media
                                .path
                                .file_name()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_default();

                            div()
                                .id(i)
                                .h(px(200.0))
                                .v_flex()
                                .p_2()
                                .gap_1()
                                .child(match thumb {
                                    Some(image) => img(image)
                                        .w_full()
                                        .h(px(160.0))
                                        .object_fit(ObjectFit::Cover)
                                        .into_any_element(),
                                    None => div()
                                        .w_full()
                                        .h(px(160.0))
                                        .child(text!("..."))
                                        .into_any_element(),
                                })
                                .child(div().w_full().flex().justify_center().child(file_name))
                        })
                        .collect()
                })
                .flex_grow(1.0)
                .into_any_element(),
            )
    }
}

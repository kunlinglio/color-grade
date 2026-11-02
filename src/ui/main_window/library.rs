use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::media;
use crate::media::comm::MediaSubscription;
use crate::media::media_type::Media;
use crate::project::comm::ProjectRuntimeHandler;
use crate::thumbnail::ThumbnailManagerClient;
use crate::ui::components::{Text, Warn};
use crate::ui::{Theme, get_theme};

#[derive(Clone, Copy)]
enum MediaSortBy {
    Path,
    Name,
    Hash,
}

/// A sorted list for storing media in a specific order.
#[derive(Clone)]
struct MediaList {
    list: Vec<Arc<Media>>,
    sort_by: MediaSortBy,
}

impl MediaList {
    fn compare_factory(sort_by: &MediaSortBy) -> fn(&Arc<Media>, &Arc<Media>) -> Ordering {
        match sort_by {
            MediaSortBy::Hash => |a, b| a.hash.cmp(&b.hash),
            MediaSortBy::Name => |a, b| a.path.file_name().cmp(&b.path.file_name()),
            MediaSortBy::Path => |a, b| a.path.cmp(&b.path),
        }
    }
    fn new(mut list: Vec<Arc<Media>>, sort_by: MediaSortBy) -> Self {
        list.sort_by(Self::compare_factory(&sort_by));
        Self { list, sort_by }
    }
    fn insert_sorted(&mut self, media: Arc<Media>) {
        let compare = Self::compare_factory(&self.sort_by);
        let index = self
            .list
            .binary_search_by(|m| compare(m, &media))
            .unwrap_or_else(|i| i);
        self.list.insert(index, media);
    }
}

pub struct Library {
    media_list: MediaList,
    /// path -> thumb
    /// NOTE: Use Path as key for thumbnail,
    /// If there are two same media in different directory, they should be showed as two media,
    /// so we cannot use hash as key.
    /// Here we cached GPU texture instead of CPU buffer "Thumbnail" to avoid expensive copy.
    thumbnails: HashMap<Arc<Media>, Arc<RenderImage>>,
    /// Pending GPU Textrue
    /// The Arc<RenderImage> cannot free it's on GPU memory.
    /// So we pending drop it until we can access window.
    to_drop: Vec<Arc<RenderImage>>,
    sort_by: MediaSortBy,
    selected: Option<Arc<Media>>,
    runtime: ProjectRuntimeHandler,
    client: Option<ThumbnailManagerClient>,
}

impl Library {
    pub fn new(runtime: ProjectRuntimeHandler) -> Self {
        let default_sort_by = MediaSortBy::Hash;
        Library {
            media_list: MediaList::new(Vec::new(), default_sort_by),
            thumbnails: HashMap::new(),
            to_drop: Vec::new(),
            sort_by: default_sort_by,
            selected: None,
            runtime,
            client: None,
        }
    }

    pub fn init(&self, cx: &mut Context<Self>, window: &mut Window) {
        cx.spawn_in(window, async move |this, cx| {
            let runtime = this
                .read_with(cx, |this, _cx| this.runtime.clone())
                .unwrap();
            // Begin scanning media
            let media_rx = runtime.listen_on_media().await;
            let thumbnail_client = runtime.subscribe_thumbnails().await;
            let _ = this.update_in(cx, |library, _window, cx| {
                library.start(cx, media_rx, thumbnail_client);
            });
        })
        .detach();
    }

    /// Monitor media events.
    fn start(
        &mut self,
        cx: &mut Context<Self>,
        mut rx: MediaSubscription,
        thumbnail_client: ThumbnailManagerClient,
    ) {
        self.client = Some(thumbnail_client);
        cx.spawn(async move |this, cx| {
            while let Some(e) = rx.get().await {
                match e {
                    media::comm::Event::Sync(list) => {
                        let _ = this.update(cx, |library, cx| {
                            library.media_list = MediaList::new(list, library.sort_by);
                            cx.notify();
                        });
                    }
                    media::comm::Event::Add(media) => {
                        let _ = this.update(cx, |library, cx| {
                            library.media_list.insert_sorted(media);
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn update_visible(&mut self, new_visible: HashSet<Arc<Media>>, cx: &mut Context<Self>) {
        let added: Vec<_> = new_visible
            .iter()
            .filter(|key| !self.thumbnails.contains_key(*key))
            .cloned()
            .collect();

        let removed: Vec<_> = self
            .thumbnails
            .keys()
            .filter(|key| !new_visible.contains(*key))
            .cloned()
            .collect();
        for media in added.into_iter() {
            cx.spawn(async move |this, cx| {
                let Ok(client) = this.read_with(cx, |this, _app| this.client.clone()) else {
                    return;
                };
                let Some(thumb) = client.unwrap().request(media.clone()).await else {
                    return;
                };
                let _ = this.update(cx, |this, cx| {
                    this.thumbnails.insert(
                        media,
                        Arc::new(RenderImage::new(vec![image::Frame::new(
                            thumb.frame.to_image(),
                        )])),
                    );
                    cx.notify();
                });
            })
            .detach();
        }
        for media in removed.into_iter() {
            if let Some(old) = self.thumbnails.remove(&media) {
                self.to_drop.push(old);
            }
        }
    }

    fn clean_texture(&mut self, window: &mut Window) {
        while let Some(t) = self.to_drop.pop() {
            let _ = window.drop_image(t);
        }
    }
}

impl Render for Library {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clean_texture(window);
        let theme = get_theme(cx);
        let medias_len = self.media_list.list.len();
        let runtime = self.runtime.clone();
        let selected = self.selected.clone();
        let this = cx.weak_entity();
        div()
            .v_flex()
            .size_full()
            .child(Text::new("Library").title())
            .child(media_list(theme, medias_len, runtime, selected, this))
    }
}

fn media_list(
    theme: Theme,
    medias_len: usize,
    runtime: ProjectRuntimeHandler,
    selected: Option<Arc<Media>>,
    this: WeakEntity<Library>,
) -> AnyElement {
    uniform_list("library_list", medias_len, move |range, window, app| {
        let rows = this
            .read_with(app, |library, _| {
                range
                    .clone()
                    .filter_map(|i| {
                        let media = library.media_list.list.get(i)?.clone();
                        let thumb = library.thumbnails.get(&media).cloned();
                        Some((media, thumb))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let visible: HashSet<Arc<Media>> = rows.iter().map(|(m, _)| m.clone()).collect();
        let this_for_visible = this.clone();
        window.defer(app, move |_window, app| {
            let _ = this_for_visible.update(app, |library, cx| {
                library.update_visible(visible, cx);
            });
        });

        rows.into_iter()
            .map(|(media, thumb)| {
                let is_selected = selected.as_ref() == Some(&media);
                let path = media.path.clone();
                let runtime = runtime.clone();
                let this = this.clone();

                let mut item = div()
                    .id(format!("media-{}", media.hash.display()))
                    .on_click(move |_event, _window, app| {
                        let _ = this.update(app, |library, cx| {
                            library.selected = Some(media.clone());
                            cx.notify();
                        });
                        runtime.open_media(&path);
                    })
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
                            .child(Warn::new("TODO: Implement loading animation"))
                            .into_any_element(),
                    });

                if is_selected {
                    item = item.bg(theme.strike_color);
                }
                item
            })
            .collect()
    })
    .flex_grow(1.0)
    .into_any_element()
}

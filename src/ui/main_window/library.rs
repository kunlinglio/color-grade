use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::media;
use crate::media::comm::MediaSubscription;
use crate::media::hash::MediaHash;
use crate::media::media_type::Media;
use crate::project::comm::ProjectRuntimeHandler;
use crate::thumbnail::ThumbnailManagerClient;
use crate::ui::components::{Text, Warn};
use crate::ui::get_theme;

#[derive(Clone, Copy)]
enum MediaSortBy {
    Path,
    Name,
    Hash,
}

/// A sorted list for storing media in a specific order.
#[derive(Clone)]
struct MediaList {
    list: Vec<Media>,
    sort_by: MediaSortBy,
}

impl MediaList {
    fn compare_factory(sort_by: &MediaSortBy) -> fn(&Media, &Media) -> Ordering {
        match sort_by {
            MediaSortBy::Hash => |a, b| a.hash.cmp(&b.hash),
            MediaSortBy::Name => |a, b| a.path.file_name().cmp(&b.path.file_name()),
            MediaSortBy::Path => |a, b| a.path.cmp(&b.path),
        }
    }
    fn new(mut list: Vec<Media>, sort_by: MediaSortBy) -> Self {
        list.sort_by(Self::compare_factory(&sort_by));
        Self { list, sort_by }
    }
    fn insert_sorted(&mut self, media: Media) {
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
    /// Used to compute diff
    last_visible: Range<usize>,
    /// path -> thumb
    /// NOTE: Use Path as key for thumbnail,
    /// If there are two same media in different directory, they should be showed as two media,
    /// so we cannot use hash as key.
    /// Here we cached GPU texture instead of CPU buffer "Thumbnail" to avoid expensive copy.
    thumbnails: HashMap<PathBuf, Arc<RenderImage>>,
    /// Pending GPU Textrue
    /// The Arc<RenderImage> cannot free it's on GPU memory.
    /// So we pending drop it until we can access window.
    to_drop: Vec<Arc<RenderImage>>,
    sort_by: MediaSortBy,
    selected: Option<MediaHash>,
    runtime: ProjectRuntimeHandler,
    client: Option<ThumbnailManagerClient>,
}

impl Library {
    pub fn new(runtime: ProjectRuntimeHandler) -> Self {
        let default_sort_by = MediaSortBy::Hash;
        Library {
            media_list: MediaList::new(Vec::new(), default_sort_by),
            last_visible: Range::from(0..0),
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

    fn update_visible(&mut self, new_visible: Range<usize>, cx: &mut Context<Self>) {
        if new_visible == self.last_visible {
            return;
        }
        let diff = range_diff(&self.last_visible, &new_visible);
        let added = diff.added;
        let removed = diff.removed;
        for index in added.into_iter().flatten() {
            let media = self.media_list.list[index].clone();
            cx.spawn(async move |this, cx| {
                let Ok(client) = this.read_with(cx, |this, _app| this.client.clone()) else {
                    return;
                };
                let Some(thumb) = client.unwrap().request(media.clone()).await else {
                    return;
                };
                let _ = this.update(cx, |this, cx| {
                    this.thumbnails.insert(
                        media.path,
                        Arc::new(RenderImage::new(vec![image::Frame::new(
                            thumb.frame.to_image(),
                        )])),
                    );
                    cx.notify();
                });
            })
            .detach();
        }
        for index in removed.into_iter().flatten() {
            let media = &self.media_list.list[index];
            if let Some(old) = self.thumbnails.remove(&media.path) {
                self.to_drop.push(old);
            }
        }
        self.last_visible = new_visible;
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
        let medias = self.media_list.clone();
        let thumbnails = self.thumbnails.clone();
        let runtime = self.runtime.clone();
        let selected = self.selected;
        let this = cx.weak_entity();
        div()
            .v_flex()
            .size_full()
            .child(Text::new("Library").title())
            .child(
                uniform_list(
                    "library_list",
                    medias.list.len(),
                    move |range, window, app| {
                        let this_cloned = this.clone();
                        let visible = range.clone();
                        if visible.len() != 1 {
                            // The uniform list often call this callback twice:
                            // First for measure element height, second for draw visible element.
                            // So we should drop first call, whose range length is always 1.
                            window.defer(app, move |_window, app| {
                                let _ = this_cloned.update(app, |library, cx| {
                                    library.update_visible(visible, cx);
                                });
                            });
                        }

                        range
                            .map(|i| {
                                let media = &medias.list[i];
                                let thumb = thumbnails.get(&media.path).cloned();
                                let is_selected = selected == Some(media.hash);
                                let path = media.path.clone();
                                let hash = media.hash;
                                let runtime = runtime.clone();
                                let this = this.clone();

                                let mut item = div()
                                    .id(i)
                                    .on_click(move |_event, _window, app| {
                                        let _ = this.update(app, |library, cx| {
                                            library.selected = Some(hash);
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
                    },
                )
                .flex_grow(1.0)
                .into_any_element(),
            )
    }
}

pub struct RangeDiff {
    pub added: Vec<Range<usize>>,
    pub removed: Vec<Range<usize>>,
}

/// Return (added, removed)
pub fn range_diff(old: &Range<usize>, new: &Range<usize>) -> RangeDiff {
    RangeDiff {
        added: subtract_range(&new, &old),
        removed: subtract_range(&old, &new),
    }
}

fn subtract_range(a: &Range<usize>, b: &Range<usize>) -> Vec<Range<usize>> {
    let mut result = Vec::with_capacity(2);
    if a.end <= b.start || b.end <= a.start {
        if !a.is_empty() {
            result.push(a.clone());
        }
        return result;
    }

    if a.start < b.start {
        result.push(a.start..b.start);
    }
    if b.end < a.end {
        result.push(b.end..a.end);
    }
    result
}

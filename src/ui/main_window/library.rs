use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::base::{Scrollbar, ScrollbarMode};
use gpui_kit::component::*;
use gpui_kit::*;
use strum::EnumIter;

use crate::media;
use crate::media::comm::MediaSubscription;
use crate::media::media_type::Media;
use crate::project::ProjectRuntimeHandler;
use crate::thumbnail::{ThumbnailManagerClient, ThumbnailSize};
use crate::ui::components::{Icon, Loading, ResizeSensitive, SelectOption, SingleSelect, Text};
use crate::ui::{Theme, get_theme};

/// The constant to control how many thumbnails will be loaded.
/// prefetched thumbs = PREFETCH_RATIO * number of Visible thumbs
const PREFETCH_RATIO: f32 = 1.0;

/// Height / width
/// Use to evaluate height for each thumbnail.
///
const PICTURE_RATIO: f32 = 3.0 / 4.0;

#[derive(Clone, Copy, EnumIter, PartialEq)]
enum MediaSortBy {
    Path,
    Name,
    Hash,
}

impl SelectOption for MediaSortBy {
    fn id(self) -> &'static str {
        match self {
            MediaSortBy::Hash => "library-sort_by-hash",
            MediaSortBy::Name => "library-sort_by-name",
            MediaSortBy::Path => "library-sort_by-path",
        }
    }
    fn label(self) -> &'static str {
        match self {
            MediaSortBy::Hash => "Hash",
            MediaSortBy::Name => "Name",
            MediaSortBy::Path => "Path",
        }
    }
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
    pub fn new(mut list: Vec<Arc<Media>>, sort_by: MediaSortBy) -> Self {
        list.sort_by(Self::compare_factory(&sort_by));
        Self { list, sort_by }
    }
    pub fn insert_sorted(&mut self, media: Arc<Media>) {
        let compare = Self::compare_factory(&self.sort_by);
        let index = self
            .list
            .binary_search_by(|m| compare(m, &media))
            .unwrap_or_else(|i| i);
        self.list.insert(index, media);
    }
    pub fn change_sort_by(&mut self, new: MediaSortBy) {
        if self.sort_by == new {
            return;
        }
        self.sort_by = new;
        self.list.sort_by(MediaList::compare_factory(&new));
    }
    pub fn replace(&mut self, by: Vec<Arc<Media>>) {
        self.list = by;
        self.list.sort_by(Self::compare_factory(&self.sort_by));
    }
}

#[derive(Clone)]
enum ThumbnailState {
    Loading(ThumbnailSize),
    Loaded(Arc<RenderImage>),
    Failed,
}

pub struct Library {
    media_list: MediaList,
    /// path -> thumb
    /// NOTE: Use Media it self as key for thumbnail,
    ///     If there are two same media in different directory, they should be showed as two media,
    ///     so we cannot use hash as key.
    ///     Here we cached GPU texture instead of CPU buffer "Thumbnail" to avoid expensive copy.
    thumbnails: HashMap<Arc<Media>, ThumbnailState>,
    /// Pending GPU Textrue
    /// The Arc<RenderImage> cannot free it's on GPU memory.
    /// So we pending drop it until we can access window.
    to_drop: Vec<Arc<RenderImage>>,
    selected: Entity<Option<Arc<Media>>>,
    runtime: ProjectRuntimeHandler,
    client: Option<ThumbnailManagerClient>,
    list_scroll_handle: UniformListScrollHandle,
    thumbnail_size: ThumbnailSize,
    width: Pixels,
}

impl Library {
    pub fn new(runtime: ProjectRuntimeHandler, selected: Entity<Option<Arc<Media>>>) -> Self {
        Library {
            media_list: MediaList::new(Vec::new(), MediaSortBy::Hash),
            thumbnails: HashMap::new(),
            to_drop: Vec::new(),
            selected: selected,
            runtime,
            client: None,
            list_scroll_handle: UniformListScrollHandle::new(),
            thumbnail_size: ThumbnailSize::Px256,
            width: px(200.),
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
                            library.media_list.replace(list);
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

    fn fetch_thumbs(&mut self, new_visible: HashSet<Arc<Media>>, cx: &mut Context<Self>) {
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
        let client = self.client.clone().unwrap();
        let size = self.thumbnail_size;
        for media in added.into_iter() {
            self.thumbnails
                .insert(media.clone(), ThumbnailState::Loading(size));
            let client = client.clone();
            cx.spawn(async move |this, cx| {
                let thumb = client.request(media.clone(), size).await;
                let _ = this.update(cx, |this, cx| {
                    let Some(slot) = this.thumbnails.get_mut(&media) else {
                        return;
                    };
                    if !matches!(*slot, ThumbnailState::Loading(current) if current == size) {
                        // Avoid cover other state
                        return;
                    }
                    *slot = match thumb {
                        Some(thumb) => ThumbnailState::Loaded(Arc::new(RenderImage::new(vec![
                            image::Frame::new(thumb.frame.to_image()),
                        ]))),
                        None => ThumbnailState::Failed,
                    };
                    cx.notify();
                });
            })
            .detach();
        }
        for media in removed.into_iter() {
            if let Some(old) = self.thumbnails.remove(&media) {
                match old {
                    ThumbnailState::Loaded(old) => {
                        self.to_drop.push(old);
                    }
                    _ => {}
                }
            }
        }
    }

    fn clean_texture(&mut self, window: &mut Window) {
        while let Some(t) = self.to_drop.pop() {
            let _ = window.drop_image(t);
        }
    }

    fn change_sort_by(&mut self, new: MediaSortBy) {
        self.media_list.change_sort_by(new);
    }

    fn change_thumbnail_size(&mut self, new: ThumbnailSize, cx: &mut Context<Self>) {
        if self.thumbnail_size == new {
            return;
        }

        self.thumbnail_size = new;
        for (_, state) in self.thumbnails.drain() {
            // TODO: Avoid show loading item when fetching new thumbs
            if let ThumbnailState::Loaded(image) = state {
                self.to_drop.push(image);
            }
        }
        cx.notify();
    }
}

impl Render for Library {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clean_texture(window);
        let theme = get_theme(cx);
        let medias_len = self.media_list.list.len();
        let runtime = self.runtime.clone();
        let selected = self.selected.read(cx).clone();
        let this = cx.weak_entity();
        let this_for_select = this.clone();
        let this_for_resize = this.clone();
        div()
            .v_flex()
            .size_full()
            .child(
                div()
                    .flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(Text::new("Library").title())
                    .child(
                        SingleSelect::<MediaSortBy>::new()
                            .select(self.media_list.sort_by)
                            .on_select(move |sort_by, _window, app| {
                                let _ = this_for_select.update(app, |this, cx| {
                                    this.change_sort_by(sort_by);
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(
                ResizeSensitive::new()
                    .size_full()
                    .on_resize(move |bounds, window, app| {
                        let width = f32::from(bounds.size.width);
                        let _ = this_for_resize.update(app, |library, cx| {
                            library.width = px(width);
                            cx.notify();
                        });
                        let size = ThumbnailSize::from_width(width);
                        let needs_update = this_for_resize
                            .read_with(app, |library, _cx| library.thumbnail_size != size)
                            .unwrap_or(false);
                        if !needs_update {
                            return;
                        }
                        let this = this_for_resize.clone();
                        window.defer(app, move |_window, app| {
                            let _ = this.update(app, |library, cx| {
                                library.change_thumbnail_size(size, cx);
                            });
                        });
                    })
                    .child(media_list(
                        theme,
                        medias_len,
                        runtime,
                        selected,
                        this,
                        self.list_scroll_handle.clone(),
                        self.width,
                    )),
            )
    }
}

fn media_list(
    theme: Theme,
    medias_len: usize,
    runtime: ProjectRuntimeHandler,
    selected: Option<Arc<Media>>,
    this: WeakEntity<Library>,
    scroll_handle: UniformListScrollHandle,
    width: Pixels,
) -> AnyElement {
    let list = uniform_list("library_list", medias_len, move |range, window, app| {
        let rows = this
            .read_with(app, |library, _| {
                range
                    .clone()
                    .map(|i| {
                        let media = library.media_list.list[i].clone();
                        let thumb = library.thumbnails.get(&media).cloned();
                        (media, thumb)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let this_for_visible = this.clone();
        if range.len() != 1 || medias_len == 1 {
            let prefetch = this
                .read_with(app, |library, _| {
                    let total = library.media_list.list.len();
                    let start = range.start.min(total);
                    let end = range.end.min(total);
                    let margin = ((end - start) as f32 * PREFETCH_RATIO * 0.5).round() as usize;
                    (start.saturating_sub(margin)..(end + margin).min(total))
                        .filter_map(|i| library.media_list.list.get(i).cloned())
                        .collect::<HashSet<_>>()
                })
                .unwrap();
            // The uniform list often call this callback twice:
            // First for measure element height, second for draw visible element.
            // So we should drop first call, whose range length is always 1.
            window.defer(app, move |_window, app| {
                let _ = this_for_visible.update(app, |library, cx| {
                    library.fetch_thumbs(prefetch, cx);
                });
            });
        }

        rows.into_iter()
            .map(|(media, thumb)| {
                let is_selected = selected.as_ref() == Some(&media);
                let path = media.path.clone();
                let runtime = runtime.clone();
                let this = this.clone();
                let card_width = (width - px(24.0)).max(px(1.0));
                let card_height = card_width * PICTURE_RATIO;

                let mut card = div()
                    .id(format!("media-{}", media.path.display()))
                    .h(card_height)
                    .p_1()
                    .w_full()
                    .flex()
                    .justify_center()
                    .rounded(theme.radii.large)
                    .on_click(move |_event, _window, app| {
                        let _ = this.update(app, |library, cx| {
                            library.selected.update(cx, |selected, cx| {
                                *selected = Some(media.clone());
                                cx.notify();
                            });
                            cx.notify();
                        });
                        runtime.open_media(&path);
                    })
                    .child(render_thumb(theme, thumb));

                if is_selected {
                    card = card.bg(theme.colors.accent.normal);
                }

                div()
                    .w_full()
                    .p_1()
                    .flex()
                    .justify_center()
                    .child(card)
                    .into_any_element()
            })
            .collect()
    })
    .track_scroll(&scroll_handle)
    .p_2()
    .size_full();

    div()
        .h_full()
        .child(list)
        .child(Scrollbar::vertical(&scroll_handle).mode(ScrollbarMode::Always))
        .into_any_element()
}

fn render_thumb(theme: Theme, thumb: Option<ThumbnailState>) -> AnyElement {
    let bg = div()
        .size_full()
        .rounded(theme.radii.small)
        .bg(theme.colors.background.surface)
        .flex()
        .items_center()
        .justify_center();
    match thumb {
        Some(ThumbnailState::Loaded(image)) => img(image)
            .h_full()
            .rounded(theme.radii.small)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        Some(ThumbnailState::Failed) => bg
            .child(
                Icon::new(assets::IconName::ImageOff)
                    .text_color(theme.colors.text.secondary)
                    .size(px(32.)),
            )
            .into_any_element(),
        Some(ThumbnailState::Loading(_)) | None => {
            bg.child(Loading::new().size(px(32.))).into_any_element()
        }
    }
}

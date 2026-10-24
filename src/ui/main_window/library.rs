use futures::channel::mpsc;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::media::Media;

pub struct Library {
    media_list: Vec<Media>,
}

impl Library {
    pub fn new() -> Self {
        Library { media_list: vec![] }
    }

    /// Start to monitor the result from RenderPipe
    pub fn start(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mut rx: mpsc::Receiver<Media>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(media) = rx.recv().await {
                let _ = cx.update(|_window, app| {
                    let _ = this.update(app, |this, cx| {
                        this.media_list.push(media);
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

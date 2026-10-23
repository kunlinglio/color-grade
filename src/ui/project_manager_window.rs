use std::path::PathBuf;

use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;

pub struct ProjectManagerWindow {}

impl ProjectManagerWindow {
    fn new() -> Self {
        ProjectManagerWindow {}
    }
    pub fn open(app: &mut App) -> Result<(AnyWindowHandle, Entity<Self>)> {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point {
                        x: 100.0.into(),
                        y: 100.0.into(),
                    },
                    size: gpui_kit::Size {
                        height: 300.0.into(),
                        width: 400.0.into(),
                    },
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("Color Grade Project Manager".into()),
                    // appears_transparent: true, // TODO: Support draw our owned title bar
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            app,
            |_window, app| app.new(|_cx| ProjectManagerWindow::new()),
        )
    }

    fn load_project(&self, window: &Window, cx: &mut Context<'_, Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose a folder for the new project".into()),
        });

        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(project_path) = paths.into_iter().next() else {
                return;
            };

            cx.update(|_, app| println!("Opened the {} as a Project!", project_path.display()))
                .ok();
        })
        .detach();
    }
}

impl Render for ProjectManagerWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::div()
            .v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .child(text!("Open a folder as a new Color Grade Project!"))
            .child(
                Button::new("load_project")
                    .primary()
                    .label("Load from a folder")
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.load_project(window, cx);
                    })),
            )
    }
}

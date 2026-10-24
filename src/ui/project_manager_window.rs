use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::ColorGradeApp;

pub struct ProjectManagerWindow {}

impl ProjectManagerWindow {
    fn new() -> Self {
        ProjectManagerWindow {}
    }
    pub fn open(app: &mut App) -> Result<(AnyWindowHandle, Entity<Self>)> {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(
                gpui_kit::Size {
                    height: 300.0.into(),
                    width: 400.0.into(),
                },
                app,
            )),
            titlebar: Some(TitlebarOptions {
                title: Some("Color Grade Project Manager".into()),
                // appears_transparent: true, // TODO: Support draw our owned title bar
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        };
        gpui_kit::open_window(options, app, |_window: &mut Window, app| {
            app.new(|_cx| ProjectManagerWindow::new())
        })
    }

    fn open_project(&self, cx: &mut Context<'_, Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose a folder for the new project".into()),
        });

        cx.spawn(async move |_window, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(project_path) = paths.into_iter().next() else {
                return;
            };

            cx.update(|app| {
                ColorGradeApp::update_global(app, |cg_app, app| {
                    cg_app.open_project(project_path, app);
                });
            });
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
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.open_project(cx);
                    })),
            )
    }
}

use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::ColorGradeApp;
use crate::ui::components::{Button, Icon, Text, TitleBar};
use crate::ui::get_theme;

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
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.), px(7.))),
            }),
            app_owns_titlebar_drag: true,
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
        let theme = get_theme(cx);
        div()
            .v_flex()
            .size_full()
            .bg(theme.colors.background.canvas)
            .child(TitleBar::new())
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .gap_2()
                    .p_2()
                    .child(
                        Text::new("Open Project")
                            .content()
                            .text_align(TextAlign::Center),
                    )
                    .child(Text::new("Open a folder as a new Color Grade Project!").content())
                    .child(
                        Button::new("load_project")
                            .primary()
                            .gap_1()
                            .child(Icon::new(IconName::FolderOpen))
                            .child(Text::new("Chose a folder"))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.open_project(cx);
                            })),
                    ),
            )
    }
}

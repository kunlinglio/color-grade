mod inspector;
mod library;
mod monitor;
mod nodes;
mod scopes;

use std::path::PathBuf;

use gpui_kit::base::StyledExt as _;
use gpui_kit::*;

use crate::app::ColorGradeApp;
use crate::project::ProjectRuntimeHandler;
use crate::ui::components::{
    Button, Icon, Text, TitleBar, h_resizable, resizable_panel, v_resizable,
};
use crate::ui::theme::ColorTheme;
use crate::ui::{get_theme, set_theme};

pub struct MainWindow {
    title: String,
    path: PathBuf,
    // Views
    monitor: Entity<monitor::Monitor>,
    library: Entity<library::Library>,
    nodes: Entity<nodes::Nodes>,
    grading: Entity<inspector::Inspector>,
    scopes: Entity<scopes::Scopes>,
    /// Handler for communicating with ProjectRuntime
    runtime: ProjectRuntimeHandler,
}

impl MainWindow {
    fn new(cx: &mut Context<Self>, runtime: ProjectRuntimeHandler, project_path: PathBuf) -> Self {
        let path = project_path.clone();

        cx.on_release(move |_, app| {
            ColorGradeApp::update_global(app, |cg_app, _| {
                cg_app.close_project(&project_path);
            });
        })
        .detach();

        let title = path
            .clone()
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or("Unknown Project".to_string());

        let monitor = cx.new(|_cx| monitor::Monitor::new());
        // Selected media: shared between multiple panels
        let selected = cx.new(|_cx| None);
        let library = cx.new(|_cx| library::Library::new(runtime.clone(), selected.clone()));
        let nodes = cx.new(|cx| nodes::Nodes::new(cx, runtime.clone(), selected.clone()));
        let grading = cx.new(|cx| inspector::Inspector::new(cx, runtime.clone(), selected.clone()));
        let scopes = cx.new(|_cx| scopes::Scopes::new(runtime.clone()));
        MainWindow {
            path,
            title,
            monitor,
            library,
            nodes,
            grading,
            scopes,
            runtime,
        }
    }

    fn init<'a>(&mut self, window: &mut Window, cx: &mut Context<'a, Self>) {
        self.monitor.update(cx, |monitor, cx| {
            monitor.init(cx, window, self.runtime.clone())
        });
        self.library
            .update(cx, |library, cx| library.init(cx, window));
        self.scopes.update(cx, |scopes, cx| scopes.init(cx, window));
    }

    pub fn open(app: &mut App, runtime: ProjectRuntimeHandler, project_path: PathBuf) {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(Bounds {
                origin: Point {
                    x: 0.0.into(),
                    y: 0.0.into(),
                },
                size: gpui_kit::Size {
                    height: 1080.0.into(),
                    width: 1920.0.into(),
                },
            })),
            titlebar: Some(TitlebarOptions {
                title: Some("Color Grade".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.), px(7.))),
            }),
            app_owns_titlebar_drag: true,
            ..Default::default()
        };
        gpui_kit::open_window(options, app, |window, app| {
            let main_window_handler = app.new(|cx| MainWindow::new(cx, runtime, project_path));
            main_window_handler.update(app, |main_window, cx| {
                main_window.init(window, cx);
            });
            main_window_handler
        })
        .expect("Failed to open MainWindow");
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let toggle_icon = if theme.colors.is_dark {
            assets::IconName::Sun
        } else {
            assets::IconName::Moon
        };
        let toggle = Button::new("theme-toggle")
            .child(Icon::new(toggle_icon).text_color(theme.colors.text.primary))
            .on_click(move |_event, _window, cx| {
                let mut current = get_theme(cx);
                set_theme(
                    if current.colors.is_dark {
                        current.colors = ColorTheme::light();
                        current
                    } else {
                        current.colors = ColorTheme::dark();
                        current
                    },
                    cx,
                );
            });
        let project_info = div()
            .flex()
            .gap_4()
            .child(Text::new(self.title.clone()).content())
            .child(Text::new(self.path.to_string_lossy()).muted());
        let title_bar = TitleBar::new().child(project_info).child(toggle);
        let library_panel = resizable_panel()
            .size_range(px(100.)..px(2400.))
            .size(px(200.))
            .child(self.library.clone());
        let monitor_panel = resizable_panel().child(self.monitor.clone());
        let nodes_panel = resizable_panel()
            .size_range(px(200.)..px(2400.))
            .child(self.nodes.clone());
        let grading_panel = resizable_panel()
            .size_range(px(200.)..px(2400.))
            .child(self.grading.clone());
        let scopes_panel = resizable_panel()
            .size_range(px(200.)..px(2400.))
            .child(self.scopes.clone());

        div()
            .v_flex()
            .size_full()
            .bg(theme.colors.background.canvas)
            .child(title_bar)
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("main-horizontal")
                        .child(library_panel)
                        .child(
                            resizable_panel().child(
                                v_resizable("middle-vertical")
                                    .child(monitor_panel)
                                    .child(nodes_panel),
                            ),
                        )
                        .child(
                            resizable_panel().size_range(px(280.)..px(2000.)).child(
                                v_resizable("right-vertical")
                                    .child(grading_panel)
                                    .child(scopes_panel),
                            ),
                        ),
                ),
            )
    }
}

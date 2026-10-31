mod grading;
mod library;
mod monitor;
mod nodes;
mod scopes;

use std::path::PathBuf;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::ColorGradeApp;
use crate::project::comm::ProjectRuntimeHandler;

pub struct MainWindow {
    // Views
    monitor: Entity<monitor::Monitor>,
    library: Entity<library::Library>,
    nodes: Entity<nodes::Nodes>,
    grading: Entity<grading::Grading>,
    scopes: Entity<scopes::Scopes>,
    /// Handler for communicating with ProjectRuntime
    runtime: ProjectRuntimeHandler,
}

impl MainWindow {
    fn new(cx: &mut Context<Self>, runtime: ProjectRuntimeHandler, project_path: PathBuf) -> Self {
        cx.on_release(move |_, app| {
            ColorGradeApp::update_global(app, |cg_app, _| {
                cg_app.close_project(&project_path);
            });
        })
        .detach();

        let monitor = cx.new(|_cx| monitor::Monitor::new());
        let library = cx.new(|_cx| library::Library::new(runtime.clone()));
        let nodes = cx.new(|_cx| nodes::Nodes::new());
        let grading = cx.new(|cx| grading::Grading::new(cx, runtime.clone()));
        let scopes = cx.new(|_cx| scopes::Scopes::new());
        MainWindow {
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
        self.library.update(cx, |library, cx| {
            library.init(cx, window, self.runtime.clone())
        });
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
                // appears_transparent: true, // TODO: Support draw our owned title bar
                appears_transparent: false,
                traffic_light_position: None,
            }),
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
    fn render(&mut self, _window: &mut Window, _self_cx: &mut Context<Self>) -> impl IntoElement {
        let library_panel = resizable_panel()
            .size_range(px(240.)..px(2400.))
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
                resizable_panel()
                    .size_range(px(280.)..px(2000.))
                    .child(
                        v_resizable("right-vertical")
                            .child(grading_panel)
                            .child(scopes_panel),
                    ),
            )
    }
}

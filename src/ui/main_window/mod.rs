mod event;
mod library;
mod monitor;
mod nodes;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::ProjectRuntime;

pub struct MainWindow {
    // Views
    monitor: Entity<monitor::Monitor>,
    library: Entity<library::Library>,
    nodes: Entity<nodes::Nodes>,
    // ProjectRuntime
    runtime: ProjectRuntime,
}

impl MainWindow {
    fn new(cx: &mut Context<Self>, runtime: ProjectRuntime) -> Self {
        let monitor = cx.new(|_cx| monitor::Monitor::new());
        let library = cx.new(|_cx| library::Library::new());
        let nodes = cx.new(|_cx| nodes::Nodes::new());
        MainWindow {
            monitor,
            library,
            nodes,
            runtime,
        }
    }

    fn init(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Make monitor subscribe the RenderPipe
        let frame_stream = self.runtime.renderer.subscribe();
        self.monitor.update(cx, |monitor, cx| {
            monitor.start(window, cx, frame_stream);
        });
        // Make the RenderPipe subscribe the event from nodes
        cx.subscribe(
            &self.nodes,
            |this, _nodes, _event: &event::RenderRequested, _cx| {
                this.runtime.renderer.submit_render();
            },
        )
        .detach();
        // Begin scanning medias
        let media_rx = self.runtime.media_manager.begin_scan();
        self.library.update(cx, |library, cx| {
            library.start(window, cx, media_rx);
        })
    }

    pub fn open(app: &mut App, runtime: ProjectRuntime) {
        gpui_kit::open_window(
            WindowOptions {
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
            },
            app,
            |window, app| {
                let main_window_handler = app.new(|cx| MainWindow::new(cx, runtime));
                main_window_handler.update(app, |main_window, cx| main_window.init(window, cx));
                main_window_handler
            },
        )
        .expect("Failed to open MainWindow");
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, _self_cx: &mut Context<Self>) -> impl IntoElement {
        let library_panel = resizable_panel()
            .size_range(px(240.)..px(2400.))
            .child(self.library.clone());
        let monitor_panel = resizable_panel().child(self.monitor.clone());
        let nodes_panel = resizable_panel().child(self.nodes.clone());
        h_resizable("left-bottom-resizable-panel")
            .child(library_panel)
            .child(
                resizable_panel().child(
                    v_resizable("top-horizontal")
                        .child(monitor_panel)
                        .child(nodes_panel),
                ),
            )
    }
}

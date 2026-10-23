mod event;
mod explorer;
mod monitor;
mod nodes;

use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;

use crate::project::ProjectRuntime;

pub struct MainWindow {
    // Views
    monitor: Entity<monitor::Monitor>,
    explorer: Entity<explorer::Explorer>,
    nodes: Entity<nodes::Nodes>,
    // ProjectRuntime
    runtime: Arc<ProjectRuntime>,
}

impl MainWindow {
    fn new(cx: &mut Context<Self>, runtime: Arc<ProjectRuntime>) -> Self {
        let monitor = cx.new(|_cx| monitor::Monitor::new());
        let explorer = cx.new(|_cx| explorer::Explorer::new());
        let nodes = cx.new(|_cx| nodes::Nodes::new());
        MainWindow {
            monitor,
            explorer,
            nodes,
            runtime,
        }
    }

    fn init(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Make monitor subscribe the RenderPipe
        let frame_stream = self.runtime.renderer.lock().unwrap().subscribe();
        self.monitor.update(cx, |monitor, cx| {
            monitor.start(window, cx, frame_stream);
        });
        // Make the RenderPipe subscribe the event from nodes
        cx.subscribe(
            &self.nodes,
            |this, _nodes, _event: &event::RenderRequested, _cx| {
                this.runtime.renderer.lock().unwrap().submit_render();
            },
        )
        .detach();
    }

    pub fn open(app: &mut App, runtime: Arc<ProjectRuntime>) {
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
        // Top-left panel: the image monitor and its controls.
        let explorer_panel = resizable_panel()
            .size_range(px(240.)..px(2400.))
            .child(self.explorer.clone());

        // Top-right panel.
        let monitor_panel = resizable_panel()
            .size_range(px(160.)..px(1600.))
            .child(self.monitor.clone());

        // Bottom panel, spanning the full width of the window.
        let nodes_panel = resizable_panel()
            .size_range(px(100.)..px(1200.))
            .child(self.nodes.clone());

        // The vertical group splits the window into the top half (left + right)
        // and the bottom, full-width panel.
        v_resizable("main-vertical")
            .child(
                resizable_panel().size_range(px(200.)..px(2400.)).child(
                    h_resizable("top-horizontal")
                        .child(explorer_panel)
                        .child(monitor_panel),
                ),
            )
            .child(nodes_panel)
    }
}

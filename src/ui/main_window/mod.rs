mod explorer;
mod monitor;
mod nodes;

use gpui_kit::component::*;
use gpui_kit::*;

pub struct MainWindow {
    monitor: Entity<monitor::Monitor>,
    explorer: Entity<explorer::Explorer>,
    nodes: Entity<nodes::Nodes>,
}

impl MainWindow {
    pub fn new(self_cx: &mut Context<Self>) -> Self {
        let monitor = self_cx.new(|_cx| monitor::Monitor::new());
        let explorer = self_cx.new(|_cx| explorer::Explorer::new());
        let nodes = self_cx.new(|_cx| nodes::Nodes::new());
        MainWindow {
            monitor,
            explorer,
            nodes,
        }
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

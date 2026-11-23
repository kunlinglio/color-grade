mod inspector;
mod library;
mod monitor;
mod nodes;
mod scopes;
mod state;

use std::path::PathBuf;

use gpui_kit::base::StyledExt as _;
use gpui_kit::*;

use crate::app::ColorGradeApp;
use crate::project::ProjectRuntimeHandler;

use super::components::{
    Button, Icon, Text, TitleBar, h_floating_resizable, resizable_panel, v_floating_resizable,
    v_resizable,
};
use super::main_window::state::EditState;
use super::theme::ColorTheme;
use super::{Theme, get_theme, set_theme};

pub struct MainWindow {
    title: String,
    path: PathBuf,
    // Views
    monitor: Entity<monitor::Monitor>,
    library: Entity<library::Library>,
    nodes: Entity<nodes::Nodes>,
    inspector: Entity<inspector::Inspector>,
    scopes: Entity<scopes::Scopes>,
    /// Handler for communicating with ProjectRuntime
    _runtime: ProjectRuntimeHandler,
}

impl MainWindow {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        runtime: ProjectRuntimeHandler,
        project_path: PathBuf,
    ) -> Self {
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

        let monitor = cx.new(|cx| monitor::Monitor::new(cx, runtime.clone()));
        let edit_state = cx.new(|_cx| EditState::new());
        let library = cx.new(|cx| library::Library::new(cx, runtime.clone(), edit_state.clone()));
        let nodes = cx.new(|cx| nodes::Nodes::new(cx, edit_state.clone()));
        let inspector = cx.new(|cx| inspector::Inspector::new(window, cx, edit_state.clone()));
        let scopes = cx.new(|cx| scopes::Scopes::new(cx, runtime.clone()));
        MainWindow {
            path,
            title,
            monitor,
            library,
            nodes,
            inspector,
            scopes,
            _runtime: runtime,
        }
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
                traffic_light_position: Some(point(px(9.), px(9.))),
            }),
            app_owns_titlebar_drag: true,
            ..Default::default()
        };
        gpui_kit::open_window(options, app, |window, app| {
            let main_window_handler =
                app.new(|cx| MainWindow::new(window, cx, runtime, project_path));
            main_window_handler
        })
        .expect("Failed to open MainWindow");
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = get_theme(cx);
        let title_bar = TitleBar::new()
            .child(
                // Project Info
                div()
                    .flex()
                    .gap_4()
                    .child(Text::new(self.title.clone()).content())
                    .child(Text::new(self.path.to_string_lossy()).muted()),
            )
            .child(
                // Toggle button
                Button::new("theme-toggle")
                    .child(
                        Icon::new(if theme.colors.is_dark {
                            assets::IconName::Sun
                        } else {
                            assets::IconName::Moon
                        })
                        .text_color(theme.colors.text.primary),
                    )
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
                    }),
            );

        div()
            .v_flex()
            .size_full()
            .bg(theme.colors.background.canvas)
            .child(title_bar)
            // Body
            .child(
                div().flex_1().min_h_0().p_1().pt_0().child(
                    h_floating_resizable("main-horizontal")
                        // Left: Library panel
                        .child(
                            resizable_panel()
                                .size_range(px(100.)..Pixels::MAX)
                                .size(px(200.))
                                .child(floating_panel(&theme).child(self.library.clone())),
                        )
                        // Middle: Monitor panel and Nodes panel
                        .child(
                            resizable_panel().px_1().child(
                                v_floating_resizable("middle-vertical")
                                    // Monitor panel
                                    .child(
                                        resizable_panel()
                                            .size_range(px(200.)..Pixels::MAX)
                                            .child(self.monitor.clone()),
                                    )
                                    // Nodes panel
                                    .child(
                                        resizable_panel()
                                            .size_range(px(200.)..Pixels::MAX)
                                            .size(px(250.))
                                            .child(
                                                floating_panel(&theme).child(self.nodes.clone()),
                                            ),
                                    ),
                            ),
                        )
                        // Right: Inspector panel and Scope panel
                        .child(
                            resizable_panel()
                                .size_range(px(280.)..px(2000.))
                                .size(px(390.))
                                .child(
                                    floating_panel(&theme).child(
                                        v_resizable("right-vertical")
                                            // Inspector panel
                                            .child(
                                                resizable_panel()
                                                    .size_range(px(200.)..Pixels::MAX)
                                                    .child(self.inspector.clone()),
                                            )
                                            // Scope panel
                                            .child(
                                                resizable_panel()
                                                    .size_range(px(200.)..Pixels::MAX)
                                                    .size(px(350.))
                                                    .child(self.scopes.clone()),
                                            ),
                                    ),
                                ),
                        ),
                ),
            )
    }
}

fn floating_panel(theme: &Theme) -> Div {
    div()
        .size_full()
        .overflow_hidden()
        .rounded(theme.radii.panel)
        .bg(theme.colors.background.surface)
}

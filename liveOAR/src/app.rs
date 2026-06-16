use crate::auth_view::{AuthOutcome, AuthView};
use crate::cluster_presets::ClusterPresetState;
use crate::live_engine::LiveEngine;
use goard_core::models::utils::secret::Secret;
use goard_core::views::main_page::dashboard::Dashboard;
use goard_core::views::main_page::gantt::GanttChart;
use goard_core::views::menu::menu::Menu;
use goard_core::views::menu::tools::Tools;
use goard_core::views::view::View;
use goard_core::models::data_structure::application_context::ApplicationContext;
use eframe::egui::{self, CentralPanel, TopBottomPanel};

pub struct App {
    pub dashboard_view: Dashboard,
    pub gantt_view: GanttChart,
    pub auth_view: AuthView,
    pub menu: Menu,
    pub secret: Secret,
    pub tools: Tools,
    pub application_context: ApplicationContext,
    cluster_presets: ClusterPresetState,
    live_engine: LiveEngine,
    auth_active: bool,
    connected_as: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let mut application_context = ApplicationContext::default();
        application_context.live_data = true;
        let mut live_engine = LiveEngine::new(chrono::Local::now());
        live_engine.update_periodically(&mut application_context);
        App {
            secret: Secret::default(),
            dashboard_view: Dashboard::default(),
            gantt_view: GanttChart::default(),
            auth_view: AuthView::default(),
            menu: Menu::default(),
            tools: Tools::default(),
            application_context,
            cluster_presets: ClusterPresetState::default(),
            live_engine,
            auth_active: true,
            connected_as: None,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.secret.update(ctx);
        self.secret.draw_snake_game(ctx);

        if self.auth_active {
            CentralPanel::default().show(ctx, |ui| {
                match self.auth_view.show(ui) {
                    Some(AuthOutcome::LoggedIn(username)) => {
                        self.connected_as = Some(username);
                        self.auth_active = false;
                    }
                    Some(AuthOutcome::ContinuedAsGuest) => {
                        self.auth_active = false;
                    }
                    None => {}
                }
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }

        let connected_as = self.connected_as.clone();
        let mut logout_clicked = false;
        let mut login_clicked = false;

        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.menu.render_with_file_items(ui, &mut self.application_context, |ui, _app| {
                if let Some(ref user) = connected_as {
                    ui.label(format!("Logged in as: {}", user));
                    ui.separator();
                    if ui.button("Logout").clicked() {
                        logout_clicked = true;
                        ui.close_menu();
                    }
                } else {
                    if ui.button("Login").clicked() {
                        login_clicked = true;
                        ui.close_menu();
                    }
                }
            });
        });

        if logout_clicked {
            self.connected_as = None;
        }
        if login_clicked {
            self.auth_view = AuthView::default();
            self.auth_active = true;
        }

        let is_admin = self.connected_as.is_some();

        TopBottomPanel::top("tool_bar").show(ctx, |ui| {
            match self.application_context.view_type {
                goard_core::views::view::ViewType::Gantt => {
                    self.tools.render_with_gantt_and_filter_extra(
                        ui,
                        &mut self.application_context,
                        &mut self.gantt_view,
                        |ui, app| {
                            ui.separator();
                            ui.label("Cluster preset:");
                            self.cluster_presets.show_selector(ui, app);
                            let manage_btn = egui::Button::new("Manage presets");
                            let resp = ui.add_enabled(is_admin, manage_btn);
                            let resp = if !is_admin {
                                resp.on_hover_text("Admin access required")
                            } else {
                                resp
                            };
                            if resp.clicked() {
                                self.cluster_presets.open_admin();
                            }
                        },
                    );
                }
                _ => {
                    self.tools.render(ui, &mut self.application_context);
                }
            }
        });

        TopBottomPanel::top("preset_admin_bar").show(ctx, |ui| {
            let cluster_names: Vec<String> = self.application_context.get_current_clusters()
                .iter().map(|c| c.name.clone()).collect();
            self.cluster_presets.show_admin(ui, &cluster_names);
        });

        self.live_engine.poll(&mut self.application_context);
        self.application_context.refresh_filters();

        if self.application_context.refresh_requested {
            self.application_context.refresh_requested = false;
            self.live_engine.instant_update(&mut self.application_context);
        }

        TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .exact_height(18.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.application_context.is_refreshing {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(egui::RichText::new(goard_core::refreshing_text()).small());
                    }
                });
            });

        CentralPanel::default().show(ctx, |ui| match self.application_context.view_type {
            goard_core::views::view::ViewType::Dashboard => {
                self.dashboard_view.render(ui, &mut self.application_context);
            }
            goard_core::views::view::ViewType::Gantt => {
                self.gantt_view.render(ui, &mut self.application_context);
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}
}

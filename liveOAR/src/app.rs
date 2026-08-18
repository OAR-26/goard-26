use crate::auth_view::{AuthOutcome, AuthView};
use crate::cluster_presets::ClusterPresetState;
use crate::live_engine::LiveEngine;
use ganttza::models::utils::secret::Secret;
use ganttza::views::main_page::dashboard::Dashboard;
use ganttza::views::main_page::gantt::GanttChart;
use ganttza::views::menu::menu::Menu;
use ganttza::views::menu::tools::Tools;
use ganttza::views::view::View;
use ganttza::models::data_structure::application_context::ApplicationContext;
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
    /// Triggers an immediate API fetch on the first main-view frame so the
    /// gantt is populated as soon as auth is dismissed (correct window known).
    first_main_frame: bool,
}

impl App {
    pub fn new() -> Self {
        let mut application_context = ApplicationContext::default();
        application_context.show_all_resources_row = true;
        #[cfg(target_arch = "wasm32")]
        if let Some(cfg) = crate::web_settings::load_gantt_config() {
            application_context.prefs.gantt_config = cfg;
            application_context.prefs.config_reload_requested = true;
        }
        let mut live_engine = LiveEngine::new(chrono::Local::now());
        live_engine.update_periodically(&mut application_context);
        #[cfg(target_arch = "wasm32")]
        let menu = Menu::with_options(crate::web_settings::load());
        #[cfg(not(target_arch = "wasm32"))]
        let menu = Menu::default();

        App {
            secret: Secret::default(),
            dashboard_view: Dashboard::default(),
            gantt_view: GanttChart::default(),
            auth_view: AuthView::default(),
            menu,
            tools: Tools::default(),
            application_context,
            cluster_presets: ClusterPresetState::default(),
            live_engine,
            auth_active: true,
            connected_as: None,
            first_main_frame: true,
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

        let mut settings_applied = false;
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            settings_applied = self.menu.render_with_extras(
                ui,
                &mut self.application_context,
                |ui, _app| {
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
                },
                |_ui, _app| {},
            );
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
                ganttza::views::view::ViewType::Gantt => {
                    self.tools.render_with_gantt_and_extras(
                        ui,
                        &mut self.application_context,
                        &mut self.gantt_view,
                        |ui, app| {
                            let current_rate = self.live_engine.refresh_rate();
                            ui.menu_button("🕓 Refresh rate", |ui| {
                                ui.set_min_width(70.0);
                                let refresh_rates = [
                                    (30, "30s"),
                                    (60, "1min"),
                                    (300, "5min"),
                                    (u64::MAX, "Never"),
                                ];
                                for (rate, label) in refresh_rates {
                                    let selected = current_rate == rate;
                                    let display_label = if selected {
                                        format!("{} ✔", label)
                                    } else {
                                        label.to_string()
                                    };
                                    if ui.selectable_label(selected, display_label).clicked() {
                                        self.live_engine.set_refresh_rate(rate);
                                        ui.close_menu();
                                    }
                                }
                            });

                            let is_refreshing = self.live_engine.is_refreshing();
                            let refresh_btn = egui::Button::new("⟳");
                            let refresh_btn_response = if is_refreshing {
                                ui.add_enabled(false, refresh_btn)
                            } else {
                                ui.add(refresh_btn)
                            };
                            if refresh_btn_response.clicked() {
                                self.live_engine.instant_update(app);
                            }
                        },
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
            let cluster_names: Vec<String> = self.application_context.data.cluster_resource_ids
                .keys().cloned().collect();
            self.cluster_presets.show_admin(ui, &cluster_names);
        });

        self.live_engine.poll(&mut self.application_context);
        self.application_context.refresh_filters();

        TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .exact_height(18.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.live_engine.is_refreshing() {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(egui::RichText::new(ganttza::refreshing_text()).small());
                    }
                });
            });

        CentralPanel::default().show(ctx, |ui| match self.application_context.view_type {
            ganttza::views::view::ViewType::Dashboard => {
                self.dashboard_view.render(ui, &mut self.application_context);
            }
            ganttza::views::view::ViewType::Gantt => {
                self.gantt_view.render(ui, &mut self.application_context);
            }
        });

        // On the first main-view frame the gantt has rendered and set the correct
        // window — fetch immediately instead of waiting 30 s for the periodic loop.
        if std::mem::take(&mut self.first_main_frame) {
            self.live_engine.instant_update(&mut self.application_context);
        }

        #[cfg(target_arch = "wasm32")]
        if settings_applied {
            crate::web_settings::save_gantt_config(&self.application_context.prefs.gantt_config);
        }

        if let Some(opts) = self.menu.take_save_request() {
            #[cfg(target_arch = "wasm32")]
            crate::web_settings::save(&opts);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = serde_json::to_string(&opts)
                .map(|json| std::fs::write("options.json", json));
        }

        // Timeline navigation (pan/zoom/jump) asked for fresher data — skip if paused.
        if self.gantt_view.take_navigation_refresh_request() && self.live_engine.refresh_rate() != u64::MAX {
            self.live_engine.instant_update(&mut self.application_context);
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}
}

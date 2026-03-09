use crate::models::data_structure::job::Job;
use crate::models::data_structure::job::JobState;
use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::utils::get_all_clusters;
use crate::models::utils::utils::get_all_hosts;
use crate::models::utils::utils::get_all_resources;
use crate::views::menu::tools::egui::Color32;
use crate::{
    models::data_structure::application_context::ApplicationContext,
    views::view::{View, ViewType},
};
use eframe::egui;

use crate::views::main_page::gantt::GanttChart;

use super::filtering::Filtering;

pub struct Tools {
    filtering_pane: Filtering,
}

impl Default for Tools {
    fn default() -> Self {
        Tools {
            filtering_pane: Filtering::default(),
        }
    }
}

/*
 * The Tools struct is a view that contains the common buttons between the Dashboard and Gantt views.
 */
impl View for Tools {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        self.render_impl(ui, app, None);
    }
}

impl Tools {
    pub fn render_with_gantt(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut ApplicationContext,
        gantt: &mut GanttChart,
    ) {
        self.render_impl(ui, app, Some(gantt));
    }

    fn render_impl(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut ApplicationContext,
        gantt: Option<&mut GanttChart>,
    ) {
        let mut gantt = gantt;
        let has_gantt = gantt.is_some();

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.set_height(25.0);

            ui.label(t!("app.mode"));

            // Dashboard Button
            let is_dashboard_selected = matches!(app.view_type, ViewType::Dashboard);

            let dashboard_btn = egui::Button::new("📊 Dashboard").frame(is_dashboard_selected);
            if ui.add(dashboard_btn).clicked() {
                app.view_type = ViewType::Dashboard;
                ui.close_menu();
                if app.all_jobs.iter().any(|job| job.id == 0) {
                    app.see_all_jobs = true;
                }

                app.all_jobs.retain(|job| job.id != 0);
            }

            // Gantt Button
            let gantt_btn = egui::Button::new("📅 Gantt").frame(!is_dashboard_selected);
            if ui.add(gantt_btn).clicked() {
                app.view_type = ViewType::Gantt;
                ui.close_menu();

                if app.see_all_jobs {
                    app.see_all_jobs = false;
                    app.all_jobs.push(Job {
                        id: 0,
                        owner: "all_resources".to_string(),
                        state: JobState::Unknown,
                        scheduled_start: 0,
                        walltime: 0,
                        hosts: get_all_hosts(&app.all_clusters),
                        clusters: get_all_clusters(&app.all_clusters),
                        command: String::new(),
                        message: None,
                        queue: String::new(),
                        assigned_resources: get_all_resources(&app.all_clusters),
                        submission_time: 0,
                        start_time: 0,
                        stop_time: 0,
                        exit_code: None,
                        gantt_color: Color32::TRANSPARENT,
                        main_resource_state: ResourceState::Unknown,
                    });
                }
            }

            // Left side: contextual controls
            ui.add_space(8.0);

            // Menu Filters
            let filters_btn =
                egui::Button::new("🔎 ".to_string() + &t!("app.menu.filters")).frame(true);
            if ui.add(filters_btn).clicked() {
                self.filtering_pane.open();
            }

            // Gantt-specific controls are part of this 2nd line.
            if let Some(gantt) = gantt.as_deref_mut() {
                gantt.render_compact_toolbar(ui, app);
            }

            // Right side: global quick actions
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Theme toggle (one click: light <-> dark)
                let is_dark = ui.ctx().style().visuals.dark_mode;
                let theme_label = if is_dark { "☀" } else { "🌙" };
                let theme_hint = if is_dark {
                    "Switch to Light"
                } else {
                    "Switch to Dark"
                };
                if ui
                    .add(egui::Button::new(theme_label))
                    .on_hover_text(theme_hint)
                    .clicked()
                {
                    app.theme_toggle_requested = true;
                }

                // Menu Refresh Rate (adjacent to theme on the right)
                ui.menu_button(
                    "🕓 ".to_string() + &t!("app.menu.refresh_rate.button"),
                    |ui| {
                        ui.set_min_width(70.0);

                        let refresh_rates = vec![
                            (30, t!("app.menu.refresh_rate.refresh_30")),
                            (60, t!("app.menu.refresh_rate.refresh_60")),
                            (300, t!("app.menu.refresh_rate.refresh_300")),
                        ];

                        for (rate, label) in refresh_rates {
                            let selected = *app.refresh_rate.lock().unwrap() == rate;
                            let display_label = if selected {
                                format!("{} ✔", label)
                            } else {
                                label.to_string()
                            };
                            if ui.selectable_label(selected, display_label).clicked() {
                                app.update_refresh_rate(rate);
                                ui.close_menu();
                            }
                        }
                    },
                );

                let refresh_btn = egui::Button::new("⟳");
                let refresh_btn_response = if *app.is_refreshing.lock().unwrap() {
                    ui.add_enabled(false, refresh_btn)
                } else {
                    ui.add(refresh_btn)
                };
                if refresh_btn_response.clicked() {
                    app.instant_update();
                }
            });

            });

            // Gantt-only: compact data summary line just below the tool bar.
            if has_gantt {
                use std::collections::HashSet;

                let total_clusters = app.all_clusters.len();
                let total_hosts: usize = app.all_clusters.iter().map(|c| c.hosts.len()).sum();

                let mut displayed_clusters: HashSet<String> = HashSet::new();
                let mut displayed_hosts: HashSet<String> = HashSet::new();
                for job in app.filtered_jobs.iter() {
                    for c in job.clusters.iter() {
                        if !c.trim().is_empty() {
                            displayed_clusters.insert(c.clone());
                        }
                    }
                    for h in job.hosts.iter() {
                        if !h.trim().is_empty() {
                            displayed_hosts.insert(h.clone());
                        }
                    }
                }

                let refreshing = *app.is_refreshing.lock().unwrap_or_else(|p| p.into_inner());
                let status = if refreshing {
                    "refreshing"
                } else if app.is_loading {
                    "loading"
                } else {
                    "ready"
                };

                ui.horizontal(|ui| {
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            let label = egui::Label::new(
                                egui::RichText::new(format!(
                                    "Data: jobs={} | clusters affichés {}/{} | hosts affichés {}/{} | {}",
                                    app.filtered_jobs.len(),
                                    displayed_clusters.len(),
                                    total_clusters,
                                    displayed_hosts.len(),
                                    total_hosts,
                                    status
                                ))
                                .text_style(egui::TextStyle::Small),
                            )
                            .truncate();
                            ui.add(label);
                        },
                    );
                });
            }

            // Show External Window
            self.filtering_pane.ui(ui, app);
        });
    }
}

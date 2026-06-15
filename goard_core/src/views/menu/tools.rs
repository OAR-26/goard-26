use crate::models::data_structure::job::Job;
use crate::models::data_structure::job::JobState;
use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::utils::get_all_clusters;
use crate::models::utils::utils::get_all_hosts;
use crate::models::utils::utils::get_all_resources;
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
                if app.data.all_jobs.iter().any(|job| job.id == 0) {
                    app.prefs.see_all_jobs = true;
                }

                app.data.all_jobs.retain(|job| job.id != 0);
            }

            // Gantt Button
            let gantt_btn = egui::Button::new("📅 Gantt").frame(!is_dashboard_selected);
            if ui.add(gantt_btn).clicked() {
                app.view_type = ViewType::Gantt;
                ui.close_menu();

                if app.prefs.see_all_jobs {
                    app.prefs.see_all_jobs = false;
                    app.data.all_jobs.push(Job {
                        id: 0,
                        owner: "all_resources".to_string(),
                        state: JobState::Unknown,
                        scheduled_start: 0,
                        walltime: 0,
                        hosts: get_all_hosts(&app.data.all_clusters),
                        clusters: get_all_clusters(&app.data.all_clusters),
                        command: String::new(),
                        message: None,
                        queue: String::new(),
                        assigned_resources: get_all_resources(&app.data.all_clusters),
                        submission_time: 0,
                        start_time: 0,
                        stop_time: 0,
                        exit_code: None,
                        main_resource_state: ResourceState::Unknown,
                        job_type: String::new(),
                        job_types: Vec::new(),
                        name: None,
                        project: String::new(),
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
                    app.prefs.theme_toggle_requested = true;
                }

                // Refresh rate + manual refresh — only relevant in live mode.
                if app.live_data {
                    ui.menu_button(
                        "🕓 ".to_string() + &t!("app.menu.refresh_rate.button"),
                        |ui| {
                            ui.set_min_width(70.0);

                            let refresh_rates = vec![
                                (30, t!("app.menu.refresh_rate.refresh_30")),
                                (60, t!("app.menu.refresh_rate.refresh_60")),
                                (300, t!("app.menu.refresh_rate.refresh_300")),
                                (u64::MAX, t!("app.menu.refresh_rate.refresh_never")),
                            ];

                            for (rate, label) in refresh_rates {
                                let selected = *app.refresh.refresh_rate.lock().unwrap() == rate;
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
                    let refresh_btn_response = if *app.refresh.is_refreshing.lock().unwrap() {
                        ui.add_enabled(false, refresh_btn)
                    } else {
                        ui.add(refresh_btn)
                    };
                    if refresh_btn_response.clicked() {
                        app.instant_update();
                    }
                }
            });

            });

            // Gantt-only: compact data summary line just below the tool bar.
            if has_gantt {
                use std::collections::HashSet;
                use crate::views::main_page::gantt::jobs::strata_field_value;

                let refreshing = *app.refresh.is_refreshing.lock().unwrap_or_else(|p| p.into_inner());
                let status = if refreshing { "refreshing" } else if app.is_loading { "loading" } else { "ready" };

                let view_name = app.prefs.current_gantt_view_name.clone();
                let levels = app.prefs.current_gantt_view_levels.clone();
                let summary_fields: Vec<String> = if app.prefs.current_gantt_view_summary_fields.is_empty() {
                    levels.last().cloned().into_iter().collect()
                } else {
                    app.prefs.current_gantt_view_summary_fields.clone()
                };

                let fields_part: String = summary_fields.iter().map(|field| {
                    let total: HashSet<String> = app.data.strata_by_resource_id.values()
                        .filter_map(|s| strata_field_value(s, field))
                        .filter(|v: &String| !v.starts_with("(no "))
                        .collect();
                    let displayed: HashSet<String> = app.data.filtered_jobs.iter()
                        .flat_map(|j| j.assigned_resources.iter())
                        .filter_map(|rid| app.data.strata_by_resource_id.get(rid))
                        .filter_map(|s| strata_field_value(s, field))
                        .filter(|v: &String| !v.starts_with("(no "))
                        .collect();
                    format!("{} {}/{}", field, displayed.len(), total.len())
                }).collect::<Vec<_>>().join(" | ");

                let view_part = if view_name.is_empty() { String::new() } else { format!("[{}] ", view_name) };

                ui.horizontal(|ui| {
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            let label = egui::Label::new(
                                egui::RichText::new(format!(
                                    "{}jobs={} | {} | {}",
                                    view_part,
                                    app.data.filtered_jobs.len(),
                                    fields_part,
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

mod canvas;
mod interaction;
mod jobs;
mod labels;
mod theme;
mod timeline;
mod types;
mod energy_plot;
mod energy_estimate;

use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::utils::{get_all_clusters, get_all_hosts, get_all_resources};
use crate::views::view::View;
use crate::{
    models::data_structure::{
        application_context::ApplicationContext,
        job::{Job, JobState},
    },
    views::components::job_details::JobDetailsWindow,
};
use chrono::{Local, TimeZone};
use eframe::egui;
use egui::{Color32, FontId, Frame, RichText, ScrollArea, Sense, Shape, TextStyle};
use std::collections::BTreeMap;

use crate::models::data_structure::application_context::ClusterPreset;
use std::collections::HashSet as StdHashSet;

#[derive(Clone, Copy, PartialEq)]
enum AdminMode {
    New,
    Modify,
}

use self::types::{gutter_stripes_total_w, Info, Options, GUTTER_WIDTH};
use self::labels::short_host_label;

fn compute_gutter_width(
    ctx: &egui::Context,
    base_font: &FontId,
    options: &Options,
    app: &ApplicationContext,
    _all_clusters: &Vec<crate::models::data_structure::cluster::Cluster>,
) -> f32 {
    let min_w = GUTTER_WIDTH;
    let n_total = options.levels.len();

    // Width for the stripe columns at the right of the gutter.
    let stripes_w = gutter_stripes_total_w(n_total);

    // Find the longest leaf-level label across all visible jobs.
    let leaf_field = options.levels.last().map(|s| s.as_str()).unwrap_or("host");
    let font_leaf = FontId::proportional((base_font.size).max(11.0));
    let mut max_label = "label".to_string();

    for job in app.filtered_jobs.iter() {
        let candidates: Vec<String> = match leaf_field {
            "host" => job.hosts.iter().map(|h| short_host_label(h)).collect(),
            "cluster" => job.clusters.clone(),
            "owner" => vec![job.owner.clone()],
            _ => job.hosts.iter().map(|h| short_host_label(h)).collect(),
        };
        for label in candidates {
            if label.len() > max_label.len() {
                max_label = label;
            }
        }
    }

    let label_left_pad = 4.0;
    let label_right_pad = 4.0;
    let label_text_w = ctx
        .fonts(|f| f.layout_no_wrap(max_label, font_leaf, Color32::BLACK).size().x);
    let label_w = label_text_w + label_left_pad + label_right_pad;

    (label_w + stripes_w).clamp(min_w, 650.0)
}

pub struct GanttChart {
    options: Options,
    job_details_windows: Vec<JobDetailsWindow>,
    // Kept so existing canvas signature compiles; no longer written to.
    collapsed_jobs_level_1: BTreeMap<String, bool>,
    collapsed_jobs_level_2: BTreeMap<(String, String), bool>,
    initial_start_s: Option<i64>,
    initial_end_s: Option<i64>,

    energy_filter_cluster: Option<String>,
    energy_filter_owner: Option<String>,

    last_canvas_usable_width_px: f32,

    admin_panel_open: bool,
    admin_mode: Option<AdminMode>,
    admin_selected_preset: Option<usize>,
    admin_original_preset_name: Option<String>,
    admin_preset_name: String,
    admin_selected_clusters: StdHashSet<String>,

    pending_navigation_refresh: bool,
}

impl Default for GanttChart {
    fn default() -> Self {
        GanttChart {
            options: Default::default(),
            job_details_windows: Vec::new(),
            collapsed_jobs_level_1: BTreeMap::new(),
            collapsed_jobs_level_2: BTreeMap::new(),
            initial_start_s: None,
            initial_end_s: None,
            last_canvas_usable_width_px: 1.0,
            admin_panel_open: false,
            admin_mode: None,
            admin_selected_preset: None,
            admin_original_preset_name: None,
            admin_preset_name: String::new(),
            admin_selected_clusters: StdHashSet::new(),
            energy_filter_cluster: None,
            energy_filter_owner: None,
            pending_navigation_refresh: false,
        }
    }
}

impl GanttChart {
    pub fn render_data_source_tabs(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        ui.add_space(4.0);

        let data_source_names = app.get_all_data_source_names();
        let current_index = app.current_data_source_index;

        ui.horizontal(|ui| {
            for (index, name) in data_source_names.iter().enumerate() {
                let is_active = index == current_index;
                let can_close = index != 0;

                let tab_color = if is_active {
                    ui.visuals().widgets.active.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                };

                let tab_text = if is_active {
                    egui::RichText::new(name).strong()
                } else {
                    egui::RichText::new(name)
                };

                let mut tab_button = egui::Button::new(tab_text).fill(tab_color).frame(true);
                if is_active {
                    tab_button = tab_button.stroke(egui::Stroke::new(
                        1.0,
                        ui.visuals().widgets.active.bg_stroke.color,
                    ));
                }

                ui.horizontal(|ui| {
                    if ui.add(tab_button).clicked() {
                        app.switch_to_data_source(index);
                    }
                    if can_close {
                        ui.add_space(-4.0);
                        let close_btn = egui::Button::new("×")
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, ui.visuals().text_color()));
                        if ui.add(close_btn).clicked() {
                            app.close_imported_data_source(index);
                        }
                    }
                });

                ui.add_space(4.0);
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
    }

    pub fn render_compact_toolbar(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        ui.menu_button(t!("app.gantt.settings.title"), |ui| {
            ui.set_max_height(500.0);
            self.options.job_color.ui(ui);
        });

        let is_admin = app.is_admin();

        let admin_button = if is_admin {
            egui::Button::new("Admin")
        } else {
            egui::Button::new("Admin")
                .fill(Color32::from_gray(110))
                .stroke(egui::Stroke::new(1.0, Color32::from_gray(170)))
        };

        let response = ui.add(admin_button);
        if !is_admin {
            response.clone().on_hover_text(
                "Accès réservé aux administrateurs. Veuillez vous authentifier.",
            );
        }
        if response.clicked() && is_admin {
            self.admin_panel_open = true;
        }

        ui.add_space(6.0);

        // Quick timeline navigation
        let base_font = TextStyle::Body.resolve(ui.style());
        let gutter_width = compute_gutter_width(
            ui.ctx(),
            &base_font,
            &self.options,
            app,
            &app.get_current_clusters(),
        );
        let fallback_usable_width = (ui.available_width() - gutter_width).max(1.0);
        let canvas_usable_width = if self.last_canvas_usable_width_px > 1.0 {
            self.last_canvas_usable_width_px
        } else {
            fallback_usable_width
        };
        let points_per_second = canvas_usable_width / self.options.canvas_width_s;
        let day_delta_s: i64 = 24 * 60 * 60;
        let week_delta_s: i64 = 7 * day_delta_s;

        ui.label(RichText::new("Nav:").text_style(TextStyle::Small));
        if ui.small_button("◀ 1w").clicked() {
            self.options.sideways_pan_in_points += week_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }
        if ui.small_button("◀ 1d").clicked() {
            self.options.sideways_pan_in_points += day_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }
        if ui.small_button("1d ▶").clicked() {
            self.options.sideways_pan_in_points -= day_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }
        if ui.small_button("1w ▶").clicked() {
            self.options.sideways_pan_in_points -= week_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }

        if ui.small_button(t!("app.gantt.now")).clicked() {
            self.options.zoom_to_relative_s_range = Some((
                ui.ctx().input(|i| i.time),
                (
                    0.,
                    (self.initial_end_s.unwrap() - self.initial_start_s.unwrap()) as f64,
                ),
            ));
            self.pending_navigation_refresh = true;
        }
    }
}

impl View for GanttChart {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        self.render_data_source_tabs(ui, app);

        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        if app.current_data_source_index == 0 {
            app.all_jobs.retain(|j| j.id != 0);
        }

        let selected_cluster_names: Option<Vec<String>> = app
            .filters
            .selected_preset
            .as_ref()
            .and_then(|n| app.cluster_presets.iter().find(|p| p.name == *n))
            .map(|p| p.clusters.clone());

        let all_hosts = if let Some(cluster_names) = &selected_cluster_names {
            app.get_current_clusters()
                .iter()
                .filter(|c| cluster_names.contains(&c.name))
                .flat_map(|c| get_all_hosts(&vec![c.clone()]))
                .collect()
        } else {
            get_all_hosts(&app.get_current_clusters())
        };

        let all_clusters = if let Some(cluster_names) = &selected_cluster_names {
            cluster_names.clone()
        } else {
            get_all_clusters(&app.get_current_clusters())
        };

        let all_resources = if let Some(cluster_names) = &selected_cluster_names {
            app.get_current_clusters()
                .iter()
                .filter(|c| cluster_names.contains(&c.name))
                .flat_map(|c| get_all_resources(&vec![c.clone()]))
                .collect()
        } else {
            get_all_resources(&app.get_current_clusters())
        };

        if app.current_data_source_index == 0 {
            app.all_jobs.push(Job {
                id: 0,
                owner: "all_resources".to_string(),
                state: JobState::Unknown,
                scheduled_start: 0,
                walltime: 0,
                hosts: all_hosts,
                clusters: all_clusters,
                command: String::new(),
                message: None,
                queue: String::new(),
                assigned_resources: all_resources,
                submission_time: 0,
                start_time: 0,
                stop_time: 0,
                exit_code: None,
                gantt_color: egui::Color32::TRANSPARENT,
                main_resource_state: ResourceState::Unknown,
            });
        }

        // Admin panel
        if self.admin_panel_open {
            let mut open = self.admin_panel_open;
            egui::Window::new("Admin configuration")
                .open(&mut open)
                .default_width(300.0)
                .show(ui.ctx(), |ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.label("Cluster presets");
                            ui.separator();

                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        self.admin_mode == Some(AdminMode::New),
                                        "New Preset",
                                    )
                                    .clicked()
                                {
                                    self.admin_mode = Some(AdminMode::New);
                                    self.admin_selected_preset = None;
                                    self.admin_original_preset_name = None;
                                    self.admin_preset_name.clear();
                                    self.admin_selected_clusters.clear();
                                }
                                if ui
                                    .selectable_label(
                                        self.admin_mode == Some(AdminMode::Modify),
                                        "Modify Preset",
                                    )
                                    .clicked()
                                {
                                    self.admin_mode = Some(AdminMode::Modify);
                                    self.admin_selected_preset = None;
                                    self.admin_original_preset_name = None;
                                    self.admin_preset_name.clear();
                                    self.admin_selected_clusters.clear();
                                }
                            });
                            ui.separator();

                            if self.admin_mode == Some(AdminMode::Modify) {
                                egui::ComboBox::from_label("Select Preset")
                                    .selected_text(
                                        self.admin_selected_preset
                                            .and_then(|i| app.cluster_presets.get(i))
                                            .map(|p| p.name.clone())
                                            .unwrap_or_else(|| "Select a preset".to_string()),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (i, preset) in app.cluster_presets.iter().enumerate() {
                                            if ui
                                                .selectable_value(
                                                    &mut self.admin_selected_preset,
                                                    Some(i),
                                                    &preset.name,
                                                )
                                                .clicked()
                                            {
                                                self.admin_original_preset_name =
                                                    Some(preset.name.clone());
                                                self.admin_preset_name = preset.name.clone();
                                                self.admin_selected_clusters =
                                                    preset.clusters.iter().cloned().collect();
                                            }
                                        }
                                    });
                                ui.separator();
                            }

                            if self.admin_mode == Some(AdminMode::New)
                                || (self.admin_mode == Some(AdminMode::Modify)
                                    && self.admin_selected_preset.is_some())
                            {
                                ui.label("Name");
                                ui.text_edit_singleline(&mut self.admin_preset_name);
                                ui.separator();
                                ui.label("Clusters to include");
                                ui.vertical(|ui| {
                                    for cluster in app.get_current_clusters() {
                                        let mut checked = self
                                            .admin_selected_clusters
                                            .contains(&cluster.name);
                                        if ui.checkbox(&mut checked, &cluster.name).changed() {
                                            if checked {
                                                self.admin_selected_clusters
                                                    .insert(cluster.name.clone());
                                            } else {
                                                self.admin_selected_clusters
                                                    .remove(&cluster.name);
                                            }
                                        }
                                    }
                                });
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if ui.button("Save").clicked()
                                        && !self.admin_preset_name.trim().is_empty()
                                    {
                                        if self.admin_mode == Some(AdminMode::Modify)
                                            && self.admin_original_preset_name.as_ref()
                                                != Some(&self.admin_preset_name)
                                        {
                                            if let Some(old_name) =
                                                &self.admin_original_preset_name
                                            {
                                                app.remove_preset(old_name);
                                            }
                                        }
                                        let preset = ClusterPreset {
                                            name: self.admin_preset_name.clone(),
                                            clusters: self
                                                .admin_selected_clusters
                                                .iter()
                                                .cloned()
                                                .collect(),
                                        };
                                        app.add_or_update_preset(preset);
                                        self.admin_mode = None;
                                        self.admin_selected_preset = None;
                                        self.admin_original_preset_name = None;
                                        self.admin_preset_name.clear();
                                        self.admin_selected_clusters.clear();
                                    }
                                    if self.admin_mode == Some(AdminMode::Modify)
                                        && self.admin_selected_preset.is_some()
                                    {
                                        if ui.button("Delete").clicked() {
                                            if let Some(i) = self.admin_selected_preset {
                                                if let Some(preset) =
                                                    app.cluster_presets.get(i)
                                                {
                                                    let name = preset.name.clone();
                                                    app.remove_preset(&name);
                                                    self.admin_mode = None;
                                                    self.admin_selected_preset = None;
                                                    self.admin_original_preset_name = None;
                                                    self.admin_preset_name.clear();
                                                    self.admin_selected_clusters.clear();
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        });
                });
            self.admin_panel_open = open;
        }

        let mut visible_range: Option<(i64, i64)> = None;
        let mut energy_points: Vec<(i64, f64)> = Vec::new();
        let mut last_gantt_usable_width_px: f32 = 1.0;
        let mut last_gantt_gutter_width_px: f32 = GUTTER_WIDTH;

        let plot_h = 270.0;
        let sep_h = 12.0;
        let gantt_h = (ui.available_height() - plot_h - sep_h).max(100.0);

        ui.allocate_ui(egui::vec2(ui.available_width(), gantt_h), |ui| {
            Frame::canvas(ui.style()).show(ui, |ui| {
                ui.visuals_mut().clip_rect_margin = 0.0;

                let fixed_timeline_y = ui.min_rect().top();
                let available_height = ui.max_rect().bottom() - ui.min_rect().bottom();

                ScrollArea::vertical().show(ui, |ui| {
                    let mut canvas = ui.available_rect_before_wrap();
                    canvas.max.y = f32::INFINITY;
                    let response =
                        ui.interact(canvas, ui.id().with("canvas"), Sense::click_and_drag());

                    let min_s = self.initial_start_s.unwrap();
                    let max_s = self.initial_end_s.unwrap();

                    let base_font = TextStyle::Body.resolve(ui.style());
                    let gutter_width = compute_gutter_width(
                        ui.ctx(),
                        &base_font,
                        &self.options,
                        app,
                        &app.get_current_clusters(),
                    );

                    let info = Info {
                        ctx: ui.ctx().clone(),
                        canvas,
                        response,
                        painter: ui.painter_at(canvas),
                        text_height: ui.text_style_height(&TextStyle::Body),
                        start_s: min_s,
                        stop_s: max_s,
                        font_id: base_font,
                        gutter_width,
                    };

                    last_gantt_usable_width_px = info.usable_width();
                    self.last_canvas_usable_width_px = info.usable_width();
                    last_gantt_gutter_width_px = gutter_width;

                    interaction::interact_with_canvas(&mut self.options, &info.response, &info);

                    let where_to_put_timeline = info.painter.add(Shape::Noop);

                    let max_y = canvas::ui_canvas(
                        &mut self.options,
                        app,
                        &info,
                        fixed_timeline_y,
                        (min_s, max_s),
                        &mut self.job_details_windows,
                        &mut self.collapsed_jobs_level_1,
                        &mut self.collapsed_jobs_level_2,
                        &app.all_clusters,
                        gutter_width,
                    );

                    let mut used_rect = canvas;
                    used_rect.max.y = max_y;
                    used_rect.max.y = used_rect.max.y.max(used_rect.min.y + available_height);

                    let timeline_shapes = timeline::paint_timeline(
                        &info,
                        used_rect,
                        &self.options,
                        min_s,
                        gutter_width,
                    );
                    info.painter
                        .set(where_to_put_timeline, Shape::Vec(timeline_shapes));

                    let current_time_line = timeline::paint_current_time_line(
                        &info,
                        &self.options,
                        used_rect,
                        gutter_width,
                    );
                    info.painter.add(current_time_line);

                    ui.allocate_rect(used_rect, Sense::hover());

                    let visible_start_s = info.start_s
                        - ((self.options.sideways_pan_in_points / info.usable_width())
                            * self.options.canvas_width_s) as i64;
                    let visible_end_s = visible_start_s + self.options.canvas_width_s as i64;
                    visible_range = Some((visible_start_s, visible_end_s));

                    let energy_jobs: Vec<Job> = app
                        .filtered_jobs
                        .iter()
                        .filter(|job| {
                            let cluster_ok = match &self.energy_filter_cluster {
                                Some(cluster) => job.clusters.iter().any(|c| c == cluster),
                                None => true,
                            };
                            let owner_ok = match &self.energy_filter_owner {
                                Some(owner) => &job.owner == owner,
                                None => true,
                            };
                            cluster_ok && owner_ok
                        })
                        .cloned()
                        .collect();

                    energy_points = energy_estimate::estimate_global_energy_series(
                        &energy_jobs,
                        visible_start_s,
                        visible_end_s,
                        10,
                        300.0,
                    );

                    let start = Local.timestamp_opt(visible_start_s, 0).unwrap();
                    let end = Local.timestamp_opt(visible_end_s, 0).unwrap();
                    app.set_localdate(start, end);

                    if self.pending_navigation_refresh {
                        let refreshing = *app
                            .is_refreshing
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        if !refreshing {
                            app.instant_update();
                            self.pending_navigation_refresh = false;
                        }
                    }
                });
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(2.0);

        if let Some((vs, ve)) = visible_range {
            let now_s = Local::now().timestamp();

            ui.horizontal_wrapped(|ui| {
                ui.label("Filtres énergie :");

                egui::ComboBox::from_id_salt("energy_filter_cluster")
                    .selected_text(
                        self.energy_filter_cluster
                            .clone()
                            .unwrap_or_else(|| "Cluster: Tous".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.energy_filter_cluster, None, "Cluster: Tous");
                        for cluster in get_all_clusters(&app.get_current_clusters()) {
                            ui.selectable_value(
                                &mut self.energy_filter_cluster,
                                Some(cluster.clone()),
                                cluster,
                            );
                        }
                    });

                let mut owners: Vec<String> =
                    app.filtered_jobs.iter().map(|j| j.owner.clone()).collect();
                owners.sort();
                owners.dedup();

                egui::ComboBox::from_id_salt("energy_filter_owner")
                    .selected_text(
                        self.energy_filter_owner
                            .clone()
                            .unwrap_or_else(|| "Owner: Tous".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.energy_filter_owner, None, "Owner: Tous");
                        for owner in owners {
                            ui.selectable_value(
                                &mut self.energy_filter_owner,
                                Some(owner.clone()),
                                owner,
                            );
                        }
                    });

                if ui.small_button("Reset").clicked() {
                    self.energy_filter_cluster = None;
                    self.energy_filter_owner = None;
                }
            });

            ui.add_space(4.0);

            let maybe_new_range = energy_plot::ui_energy_global(
                ui,
                &energy_points,
                vs,
                ve,
                now_s,
                last_gantt_gutter_width_px,
            );

            if let Some((new_vs, new_ve)) = maybe_new_range {
                let new_width_s = (new_ve - new_vs).max(1) as f32;
                self.options.canvas_width_s = new_width_s;
                let start_s = self.initial_start_s.unwrap();
                let canvas_w_px = last_gantt_usable_width_px.max(1.0);
                let pan_px =
                    -(((new_vs - start_s) as f32) / self.options.canvas_width_s) * canvas_w_px;
                self.options.sideways_pan_in_points = pan_px;
                self.pending_navigation_refresh = true;
            }
        }

        self.job_details_windows.retain(|w| w.is_open());
        for window in self.job_details_windows.iter_mut() {
            window.ui(ui);
        }
    }
}

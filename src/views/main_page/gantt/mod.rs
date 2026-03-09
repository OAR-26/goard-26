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
    views::components::{
        gantt_aggregate_by::{AggregateByLevel1Enum, AggregateByLevel2Enum},
        job_details::JobDetailsWindow,
    },
};
use chrono::{Local, TimeZone};
use eframe::egui;
use egui::{Color32, FontId, Frame, RichText, ScrollArea, Sense, Shape, TextStyle};
use std::collections::BTreeMap;

use crate::models::data_structure::application_context::ClusterPreset;
use std::collections::HashSet as StdHashSet; // to avoid confusion with earlier import

#[derive(Clone, Copy, PartialEq)]
enum AdminMode {
    New,
    Modify,
}

use self::types::{gutter_g5k_total_w, Info, Options, GUTTER_WIDTH};
use self::labels::short_host_label;

fn compute_gutter_width(
    ctx: &egui::Context,
    base_font: &FontId,
    options: &Options,
    app: &ApplicationContext,
    _all_clusters: &Vec<crate::models::data_structure::cluster::Cluster>,
) -> f32 {
    let min_w = GUTTER_WIDTH;

    // Grid5000-like view: Cluster -> Host.
    let is_grid5000 = options.aggregate_by.level_1 == AggregateByLevel1Enum::Cluster
        && options.aggregate_by.level_2 == AggregateByLevel2Enum::Host;
    if is_grid5000 {
        let font_host = FontId::proportional((base_font.size).max(11.0));
        let mut max_host = "host".to_string();

        // Only consider displayed hosts.
        for job in app.filtered_jobs.iter() {
            for host in job.hosts.iter() {
                let host_short = short_host_label(host);
                if host_short.len() > max_host.len() {
                    max_host = host_short;
                }
            }
        }

        let label_left_pad = 4.0;
        let label_right_pad = 4.0;
        let host_text_w = ctx
            .fonts(|f| f.layout_no_wrap(max_host, font_host, Color32::BLACK).size().x);
        let host_w = host_text_w + label_left_pad + label_right_pad;

        let stripes_w = gutter_g5k_total_w();

        return (host_w + stripes_w).min(650.0);
    }

    // Generic gutter: based on the widest visible label.
    let mut max_label = "label".to_string();
    for job in app.filtered_jobs.iter() {
        match options.aggregate_by.level_1 {
            AggregateByLevel1Enum::Owner => {
                if job.owner.len() > max_label.len() {
                    max_label = job.owner.clone();
                }
            }
            AggregateByLevel1Enum::Host => {
                for host in job.hosts.iter() {
                    if host.len() > max_label.len() {
                        max_label = host.clone();
                    }
                }
                if options.aggregate_by.level_2 == AggregateByLevel2Enum::Owner
                    && job.owner.len() > max_label.len()
                {
                    max_label = job.owner.clone();
                }
            }
            AggregateByLevel1Enum::Cluster => {
                for cluster in job.clusters.iter() {
                    if cluster.len() > max_label.len() {
                        max_label = cluster.clone();
                    }
                }
                if options.aggregate_by.level_2 == AggregateByLevel2Enum::Owner
                    && job.owner.len() > max_label.len()
                {
                    max_label = job.owner.clone();
                }
            }
        }
    }

    let text_w = ctx
        .fonts(|f| f.layout_no_wrap(max_label, base_font.clone(), Color32::BLACK).size().x)
        + 60.0; // indentation + padding

    text_w.clamp(min_w, 520.0)
}

pub struct GanttChart {
    options: Options,
    job_details_windows: Vec<JobDetailsWindow>,
    collapsed_jobs_level_1: BTreeMap<String, bool>,
    collapsed_jobs_level_2: BTreeMap<(String, String), bool>,
    initial_start_s: Option<i64>,
    initial_end_s: Option<i64>,
    last_canvas_usable_width_px: f32,

    last_aggregate_by: (AggregateByLevel1Enum, AggregateByLevel2Enum),

    // admin panel state
    admin_panel_open: bool,
    admin_mode: Option<AdminMode>,
    admin_selected_preset: Option<usize>,
    admin_original_preset_name: Option<String>,
    admin_preset_name: String,
    admin_selected_clusters: StdHashSet<String>,
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

            last_aggregate_by: (AggregateByLevel1Enum::Cluster, AggregateByLevel2Enum::Host),

            admin_panel_open: false,
            admin_mode: None,
            admin_selected_preset: None,
            admin_original_preset_name: None,
            admin_preset_name: String::new(),
            admin_selected_clusters: StdHashSet::new(),
        }
    }
}

impl GanttChart {
    pub fn render_compact_toolbar(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        // Ensure we have initial bounds for Center-on-now.
        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        ui.menu_button(t!("app.gantt.settings.title"), |ui| {
            ui.set_max_height(500.0);

            let before = (self.options.aggregate_by.level_1, self.options.aggregate_by.level_2);
            self.options.aggregate_by.ui(ui);
            let after = (self.options.aggregate_by.level_1, self.options.aggregate_by.level_2);
            if after != before || after != self.last_aggregate_by {
                self.last_aggregate_by = after;
                self.collapsed_jobs_level_1.clear();
                self.collapsed_jobs_level_2.clear();
                self.job_details_windows.clear();
                self.options.current_hovered_job = None;
                self.options.previous_hovered_job = None;
                self.options.current_hovered_resource_state = None;
                self.options.current_hovered_resource_label = None;
            }
            ui.separator();

            // Grid5000: compact rows forced.
            self.options.compact_rows = true;

            self.options.job_color.ui(ui);
        });

        // show admin panel button only for admin users
        if app.is_admin() {
            if ui.small_button("Admin").clicked() {
                self.admin_panel_open = true;
            }
        }

        ui.add_space(6.0);

        // Timeline navigation
        let base_font = TextStyle::Body.resolve(ui.style());
        let gutter_width =
            compute_gutter_width(ui.ctx(), &base_font, &self.options, app, &app.all_clusters);
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
        }
        if ui.small_button("◀ 1d").clicked() {
            self.options.sideways_pan_in_points += day_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
        }
        if ui.small_button("1d ▶").clicked() {
            self.options.sideways_pan_in_points -= day_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
        }
        if ui.small_button("1w ▶").clicked() {
            self.options.sideways_pan_in_points -= week_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
        }

        if ui.small_button(t!("app.gantt.now")).clicked() {
            self.options.zoom_to_relative_s_range = Some((
                ui.ctx().input(|i| i.time),
                (
                    0.,
                    (self.initial_end_s.unwrap() - self.initial_start_s.unwrap()) as f64,
                ),
            ));
        }
    }
}

impl View for GanttChart {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        // Toolbar is rendered in the global tool bar; keep this view focused on the chart.

        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        // Always show all resources, filtered by preset if selected
        // Remove any existing all_resources job and re-add with current preset
        app.all_jobs.retain(|j| j.id != 0);

        let selected_cluster_names: Option<Vec<String>> = app.filters.selected_preset.as_ref()
            .and_then(|preset_name| app.cluster_presets.iter().find(|p| p.name == *preset_name))
            .map(|preset| preset.clusters.clone());

        let all_hosts = if let Some(cluster_names) = &selected_cluster_names {
            app.all_clusters.iter()
                .filter(|c| cluster_names.contains(&c.name))
                .flat_map(|c| get_all_hosts(&vec![c.clone()]))
                .collect()
        } else {
            get_all_hosts(&app.all_clusters)
        };

        let all_clusters = if let Some(cluster_names) = &selected_cluster_names {
            cluster_names.clone()
        } else {
            get_all_clusters(&app.all_clusters)
        };

        let all_resources = if let Some(cluster_names) = &selected_cluster_names {
            app.all_clusters.iter()
                .filter(|c| cluster_names.contains(&c.name))
                .flat_map(|c| get_all_resources(&vec![c.clone()]))
                .collect()
        } else {
            get_all_resources(&app.all_clusters)
        };

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

        // (Toolbar is rendered in the global tool bar.)

        // admin panel window
        if self.admin_panel_open {
            // avoid borrow conflict by using a temporary
            let mut open = self.admin_panel_open;
            egui::Window::new("Admin configuration")
                .open(&mut open)
                .default_width(300.0)
                .show(ui.ctx(), |ui| {
                    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.label("Cluster presets");
                        ui.separator();

                        ui.horizontal(|ui| {
                            if ui.selectable_label(self.admin_mode == Some(AdminMode::New), "New Preset").clicked() {
                                self.admin_mode = Some(AdminMode::New);
                                self.admin_selected_preset = None;
                                self.admin_original_preset_name = None;
                                self.admin_preset_name.clear();
                                self.admin_selected_clusters.clear();
                            }
                            if ui.selectable_label(self.admin_mode == Some(AdminMode::Modify), "Modify Preset").clicked() {
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
                                            .selectable_value(&mut self.admin_selected_preset, Some(i), &preset.name)
                                            .clicked()
                                        {
                                            self.admin_original_preset_name = Some(preset.name.clone());
                                            self.admin_preset_name = preset.name.clone();
                                            self.admin_selected_clusters =
                                                preset.clusters.iter().cloned().collect();
                                        }
                                    }
                                });
                            ui.separator();
                        }

                        // Show form only if mode is selected and for modify, preset is selected
                        if self.admin_mode == Some(AdminMode::New) || (self.admin_mode == Some(AdminMode::Modify) && self.admin_selected_preset.is_some()) {
                            ui.label("Name");
                            ui.text_edit_singleline(&mut self.admin_preset_name);
                            ui.separator();
                            ui.label("Clusters to include");
                            ui.vertical(|ui| {
                                for cluster in &app.all_clusters {
                                    let mut checked = self
                                        .admin_selected_clusters
                                        .contains(&cluster.name);
                                    if ui.checkbox(&mut checked, &cluster.name).changed() {
                                        if checked {
                                            self.admin_selected_clusters.insert(cluster.name.clone());
                                        } else {
                                            self.admin_selected_clusters.remove(&cluster.name);
                                        }
                                    }
                                }
                            });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("Save").clicked() {
                                    if !self.admin_preset_name.trim().is_empty() {
                                        // If modifying and name changed, remove the old preset first
                                        if self.admin_mode == Some(AdminMode::Modify) && self.admin_original_preset_name.as_ref() != Some(&self.admin_preset_name) {
                                            if let Some(old_name) = &self.admin_original_preset_name {
                                                app.remove_preset(old_name);
                                            }
                                        }
                                        let preset = ClusterPreset {
                                            name: self.admin_preset_name.clone(),
                                            clusters: self.admin_selected_clusters.iter().cloned().collect(),
                                        };
                                        app.add_or_update_preset(preset);
                                        // Reset to initial state
                                        self.admin_mode = None;
                                        self.admin_selected_preset = None;
                                        self.admin_original_preset_name = None;
                                        self.admin_preset_name.clear();
                                        self.admin_selected_clusters.clear();
                                    }
                                }
                                if self.admin_mode == Some(AdminMode::Modify) && self.admin_selected_preset.is_some() {
                                    if ui.button("Delete").clicked() {
                                        if let Some(i) = self.admin_selected_preset {
                                            if let Some(preset) = app.cluster_presets.get(i) {
                                                let name = preset.name.clone();
                                                app.remove_preset(&name);
                                                // Reset to initial state
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

        let plot_h = 180.0;
        let sep_h = 8.0;

        // réserve une hauteur pour le gantt = hauteur restante - plot
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
                    let gutter_width =
                        compute_gutter_width(ui.ctx(), &base_font, &self.options, app, &app.all_clusters);

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

                    let timeline_shapes =
                        timeline::paint_timeline(&info, used_rect, &self.options, min_s, gutter_width);
                    info.painter.set(where_to_put_timeline, Shape::Vec(timeline_shapes));

                    let current_time_line =
                        timeline::paint_current_time_line(&info, &self.options, used_rect, gutter_width);
                    info.painter.add(current_time_line);

                    ui.allocate_rect(used_rect, Sense::hover());

                    // --- calcul fenêtre visible + énergie
                    let visible_start_s = info.start_s
                        - ((self.options.sideways_pan_in_points / info.usable_width())
                            * self.options.canvas_width_s) as i64;
                    let visible_end_s = visible_start_s + self.options.canvas_width_s as i64;

                    visible_range = Some((visible_start_s, visible_end_s));

                    energy_points = energy_estimate::estimate_global_energy_series(
                        &app.filtered_jobs,
                        visible_start_s,
                        visible_end_s,
                        10,     // 1 point toutes les 10 secondes
                        300.0,  // watts par unité (ici unité ~ host si assigned_resources est vide)
                    );

                    let (mut mn, mut mx) = (f64::INFINITY, f64::NEG_INFINITY);
                    for (_, w) in &energy_points {
                        mn = mn.min(*w);
                        mx = mx.max(*w);
                    }
                    println!("energy_points: n={} min={} max={}", energy_points.len(), mn, mx);

                    let start = Local.timestamp_opt(visible_start_s, 0).unwrap();
                    let end = Local.timestamp_opt(visible_end_s, 0).unwrap();
                    app.set_localdate(start, end);
                });
            });
        });

        // --- zone plot FIXE en dessous ---
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(2.0);

        if let Some((vs, ve)) = visible_range {
            let now_s = Local::now().timestamp();

            if let Some((new_vs, new_ve)) =
                energy_plot::ui_energy_global(ui, &energy_points, vs, ve, now_s, last_gantt_gutter_width_px)
            {
                let new_width_s = (new_ve - new_vs).max(1) as f32;
        
                // zoom gantt = largeur en secondes visible
                self.options.canvas_width_s = new_width_s;
        
                // recalculer le pan pour que visible_start_s devienne new_vs
                // visible_start = start_s + ( -pan_px / canvas_w_px ) * canvas_width_s
                // => pan_px = -((visible_start - start_s)/canvas_width_s) * usable_width_px
                let start_s = self.initial_start_s.unwrap();

                // IMPORTANT: idéalement il faut la width réelle du canvas gantt.
                // ici on prend une approximation: largeur disponible du ui du bas.
                let canvas_w_px = last_gantt_usable_width_px.max(1.0);

                let pan_px =
                    -(((new_vs - start_s) as f32) / self.options.canvas_width_s) * canvas_w_px;
                self.options.sideways_pan_in_points = pan_px;
            }
        }

        self.job_details_windows.retain(|w| w.is_open());
        for window in self.job_details_windows.iter_mut() {
            window.ui(ui);
        }
    }
}

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
use chrono::{Local, TimeZone, Duration};
use eframe::egui;
use egui::{Color32, FontId, Frame, RichText, ScrollArea, Sense, Shape, TextStyle};
use std::collections::{BTreeMap, HashSet};

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

    last_aggregate_by: (AggregateByLevel1Enum, AggregateByLevel2Enum),
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

            last_aggregate_by: (AggregateByLevel1Enum::Cluster, AggregateByLevel2Enum::Host),
        }
    }
}

impl View for GanttChart {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        ui.heading(RichText::new(t!("app.gantt.title")).strong());

        // Completeness: displayed (filtered jobs) vs loaded (resources).
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

        ui.label(format!(
            "Data: jobs={} | clusters affichés {}/{} | hosts affichés {}/{} | {}",
            app.filtered_jobs.len(),
            displayed_clusters.len(),
            total_clusters,
            displayed_hosts.len(),
            total_hosts,
            status
        ));

        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        ui.horizontal(|ui| {
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

                if (self.options.aggregate_by.level_1 != AggregateByLevel1Enum::Owner
                    && self.options.aggregate_by.level_2 == AggregateByLevel2Enum::None)
                    || (self.options.aggregate_by.level_1 == AggregateByLevel1Enum::Cluster
                        && self.options.aggregate_by.level_2 == AggregateByLevel2Enum::Host)
                {
                    if ui
                        .checkbox(
                            &mut self.options.see_all_res,
                            t!("app.gantt.settings.show_resources"),
                        )
                        .clicked()
                    {
                        if self.options.see_all_res {
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
                                gantt_color: egui::Color32::TRANSPARENT,
                                main_resource_state: ResourceState::Unknown,
                            });
                        } else {
                            app.all_jobs.retain(|job| job.id != 0);
                        }
                    }
                    ui.separator();
                } else {
                    if self.options.see_all_res {
                        app.all_jobs.retain(|job| job.id != 0);
                    }
                    self.options.see_all_res = false;
                }

                // Grid5000: compact rows forced.
                self.options.compact_rows = true;

                self.options.job_color.ui(ui);
            });

            ui.menu_button(" ?", |ui| {
                ui.label(t!("app.gantt.help"));
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t!("app.gantt.now")).clicked() {
                    self.options.zoom_to_relative_s_range = Some((
                        ui.ctx().input(|i| i.time),
                        (
                            0.,
                            (self.initial_end_s.unwrap() - self.initial_start_s.unwrap()) as f64,
                        ),
                    ));
                }
            });
        });

        // Timeline navigation bar
        ui.horizontal(|ui| {
            let base_font = TextStyle::Body.resolve(ui.style());
            let gutter_width = compute_gutter_width(ui.ctx(), &base_font, &self.options, app, &app.all_clusters);
            let usable_width = ui.available_width() - gutter_width;
            let points_per_second = usable_width / self.options.canvas_width_s;
            let min_s = self.initial_start_s.unwrap();
            let current_visible_s = min_s - (self.options.sideways_pan_in_points * self.options.canvas_width_s / usable_width) as i64;
            let current_local = chrono::DateTime::from_timestamp(current_visible_s, 0).unwrap().with_timezone(&chrono::Local);
            let next_day_local = current_local + Duration::days(1);
            let day_delta_s = next_day_local.timestamp() - current_local.timestamp();
            let next_week_local = current_local + Duration::days(7);
            let week_delta_s = next_week_local.timestamp() - current_local.timestamp();

            ui.label("Navigate:");
            if ui.button("◀ 1w").clicked() {
                self.options.sideways_pan_in_points += week_delta_s as f32 * points_per_second;
                self.options.zoom_to_relative_s_range = None;
            }
            if ui.button("◀ 1d").clicked() {
                self.options.sideways_pan_in_points += day_delta_s as f32 * points_per_second;
                self.options.zoom_to_relative_s_range = None;
            }
            if ui.button("1d ▶").clicked() {
                self.options.sideways_pan_in_points -= day_delta_s as f32 * points_per_second;
                self.options.zoom_to_relative_s_range = None;
            }
            if ui.button("1w ▶").clicked() {
                self.options.sideways_pan_in_points -= week_delta_s as f32 * points_per_second;
                self.options.zoom_to_relative_s_range = None;
            }
        });

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

mod canvas;
mod interaction;
mod jobs;
mod labels;
mod theme;
mod timeline;
mod types;

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
use egui::{Frame, RichText, ScrollArea, Sense, Shape, TextStyle};
use std::collections::BTreeMap;

use self::types::{Info, Options, GUTTER_WIDTH};

pub struct GanttChart {
    options: Options,
    job_details_windows: Vec<JobDetailsWindow>,
    collapsed_jobs_level_1: BTreeMap<String, bool>,
    collapsed_jobs_level_2: BTreeMap<(String, String), bool>,
    initial_start_s: Option<i64>,
    initial_end_s: Option<i64>,
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
        }
    }
}

impl View for GanttChart {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        ui.heading(RichText::new(t!("app.gantt.title")).strong());

        let reset_view = false;

        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        ui.horizontal(|ui| {
            ui.menu_button(t!("app.gantt.settings.title"), |ui| {
                ui.set_max_height(500.0);

                self.options.aggregate_by.ui(ui);
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

                ui.checkbox(
                    &mut self.options.squash_resources,
                    t!("app.gantt.settings.squash_resources"),
                );
                ui.separator();

                ui.checkbox(
                    &mut self.options.compact_rows,
                    "Compact (Grid5000)",
                );
                ui.separator();

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

        Frame::canvas(ui.style()).show(ui, |ui| {
            ui.visuals_mut().clip_rect_margin = 0.0;

            let fixed_timeline_y = ui.min_rect().top();

            let available_height = ui.max_rect().bottom() - ui.min_rect().bottom();
            ScrollArea::vertical().show(ui, |ui| {
                let mut canvas = ui.available_rect_before_wrap();
                canvas.max.y = f32::INFINITY;
                let response = ui.interact(canvas, ui.id().with("canvas"), Sense::click_and_drag());

                let min_s = self.initial_start_s.unwrap();
                let max_s = self.initial_end_s.unwrap();

                let info = Info {
                    ctx: ui.ctx().clone(),
                    canvas,
                    response,
                    painter: ui.painter_at(canvas),
                    text_height: app.font_size as f32,
                    start_s: min_s,
                    stop_s: max_s,
                    font_id: TextStyle::Body.resolve(ui.style()),
                    gutter_width: GUTTER_WIDTH,
                };

                if reset_view {
                    self.options.zoom_to_relative_s_range = Some((
                        info.ctx.input(|i| i.time),
                        (0., (info.stop_s - info.start_s) as f64),
                    ));
                }

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
                    GUTTER_WIDTH,
                );

                let mut used_rect = canvas;
                used_rect.max.y = max_y;

                used_rect.max.y = used_rect.max.y.max(used_rect.min.y + available_height);

                let timeline_shapes =
                    timeline::paint_timeline(&info, used_rect, &self.options, min_s, GUTTER_WIDTH);
                info.painter
                    .set(where_to_put_timeline, Shape::Vec(timeline_shapes));

                let current_time_line =
                    timeline::paint_current_time_line(&info, &self.options, used_rect, GUTTER_WIDTH);
                info.painter.add(current_time_line);

                ui.allocate_rect(used_rect, Sense::hover());

                {
                    let visible_start_s = info.start_s
                        + ((-self.options.sideways_pan_in_points / info.canvas.width())
                            * self.options.canvas_width_s) as i64;
                    let visible_end_s = visible_start_s + self.options.canvas_width_s as i64;

                    let start = Local.timestamp_opt(visible_start_s, 0).unwrap();
                    let end = Local.timestamp_opt(visible_end_s, 0).unwrap();

                    app.set_localdate(start, end);
                }
            });
        });

        self.job_details_windows.retain(|w| w.is_open());
        for window in self.job_details_windows.iter_mut() {
            window.ui(ui);
        }
    }
}

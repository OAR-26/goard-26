use crate::models::data_structure::cluster::Cluster;
use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::date_converter::format_timestamp;
use crate::models::utils::utils::cluster_contain_host;
use crate::models::utils::utils::compare_string_with_number;
use crate::models::utils::utils::contains_cluster;
use crate::models::utils::utils::contains_host;
use crate::models::utils::utils::get_all_clusters;
use crate::models::utils::utils::get_all_hosts;
use crate::models::utils::utils::get_all_resources;
use crate::models::utils::utils::get_cluster_from_name;
use crate::models::utils::utils::get_cluster_state_from_name;
use crate::models::utils::utils::get_host_state_from_name;
use crate::models::utils::utils::get_tree_structure_for_job;
use crate::views::view::View;
use crate::{
    models::data_structure::{
        application_context::ApplicationContext,
        job::{Job, JobState},
    },
    views::components::{
        gantt_aggregate_by::{AggregateBy, AggregateByLevel1Enum, AggregateByLevel2Enum},
        gantt_job_color::JobColor,
        job_details::JobDetailsWindow,
    },
};
use chrono::{DateTime, Local, TimeZone};
use eframe::egui;
use egui::{
    lerp, pos2, remap_clamp, Align2, Color32, CursorIcon, FontId, Frame, Id, LayerId,
    Order, PointerButton, Pos2, Rect, Response, Rgba, RichText, ScrollArea, Sense, Shape, Stroke,
    TextStyle,
};
use std::collections::BTreeMap;

/**
 * GanttChart struct
 */
pub struct GanttChart {
    options: Options,                           // options for the GanttChart
    job_details_windows: Vec<JobDetailsWindow>, // job details windows

    // Tracks which top-level categories (e.g., owners, hosts, or clusters) are collapsed in the Gantt view
    // Key: The category name (e.g., "user1" for owner, "host1" for host)
    // Value: true if collapsed (hidden), false if expanded (visible)
    collapsed_jobs_level_1: BTreeMap<String, bool>,

    // Tracks which second-level subcategories are collapsed within their parent categories
    // Key: Tuple of (parent_category, subcategory) - e.g., ("cluster1", "host1")
    // Value: true if collapsed (hidden), false if expanded (visible)
    collapsed_jobs_level_2: BTreeMap<(String, String), bool>,

    initial_start_s: Option<i64>, // Initial start timestamp
    initial_end_s: Option<i64>, // Initial end timestamp
}

/**
 * Default implementation for the GanttChart
 */
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

/**
 * Implementation of the View trait for the GanttChart
 */
impl View for GanttChart {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        ui.heading(RichText::new(t!("app.gantt.title")).strong());

        let reset_view = false;

        // Initialize initial timestamps if not already done
        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        // Settings menu
        ui.horizontal(|ui| {
            ui.menu_button(t!("app.gantt.settings.title"), |ui| {
                ui.set_max_height(500.0);

                // Aggregate by component (levels)
                self.options.aggregate_by.ui(ui);
                ui.separator();

                // If last aggregation level is set to cluster or host
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
                                gantt_color: Color32::TRANSPARENT,
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

                // Job color component (random, state)
                self.options.job_color.ui(ui);
            });

            // Help button
            ui.menu_button(" ?", |ui| {
                ui.label(t!("app.gantt.help"));
            });

            // Reset to now button
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

        // Canvas
        Frame::canvas(ui.style()).show(ui, |ui| {
            ui.visuals_mut().clip_rect_margin = 0.0;

            // Calculate the y-coordinate of the fixed timeline
            let fixed_timeline_y = ui.min_rect().top();

            let available_height = ui.max_rect().bottom() - ui.min_rect().bottom();
            ScrollArea::vertical().show(ui, |ui| {
                let mut canvas = ui.available_rect_before_wrap();
                canvas.max.y = f32::INFINITY;
                let response = ui.interact(canvas, ui.id().with("canvas"), Sense::click_and_drag());

                let min_s = self.initial_start_s.unwrap();
                let max_s = self.initial_end_s.unwrap();

                // Initialize canvas info
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

                // When we reset the view, we want to zoom to the full range
                if reset_view {
                    self.options.zoom_to_relative_s_range = Some((
                        info.ctx.input(|i| i.time),
                        (0., (info.stop_s - info.start_s) as f64),
                    ));
                }

                // Interact with the canvas
                interact_with_canvas(&mut self.options, &info.response, &info);

                // Put the timeline
                let where_to_put_timeline = info.painter.add(Shape::Noop);

                // Paint the canvas
                let max_y = ui_canvas(
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

                // Fill out space that we don't use so that the `ScrollArea` doesn't collapse in height:
                used_rect.max.y = used_rect.max.y.max(used_rect.min.y + available_height);

                let timeline = paint_timeline(&info, used_rect, &self.options, min_s, GUTTER_WIDTH);
                info.painter
                    .set(where_to_put_timeline, Shape::Vec(timeline));

                // Adding a line to show the current time AFTER all other elements
                let current_time_line = paint_current_time_line(&info, &self.options, used_rect, GUTTER_WIDTH);
                info.painter.add(current_time_line);

                ui.allocate_rect(used_rect, Sense::hover());

                {
                    // Calculate the visible time range from the canvas parameters
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

        // Part to display the details of a job when clicked
        self.job_details_windows.retain(|w| w.is_open());

        // Display job detail windows
        for window in self.job_details_windows.iter_mut() {
            window.ui(ui);
        }
    }
}

/****************************************************************************************************************************/
// CANVAS INFO
/****************************************************************************************************************************/

struct Info {
    ctx: egui::Context,     // context
    canvas: Rect,           // canvas to paint
    response: Response,     // response from the canvas
    painter: egui::Painter, // painter for the canvas
    text_height: f32,       // height of a line of text
    start_s: i64,           // start time in seconds
    stop_s: i64,            // stop time in seconds
    font_id: FontId,        // font id
    gutter_width: f32,      // fixed left gutter width for labels
}

impl Info {
    fn usable_width(&self) -> f32 {
        (self.canvas.width() - self.gutter_width).max(1.0)
    }

    /**
     * Returns the x-coordinate (in points from the canvas) to the given timestamp
     */
    fn point_from_s(&self, options: &Options, ns: i64) -> f32 {
        // Time axis starts after the fixed gutter
        self.canvas.min.x
            + self.gutter_width
            + options.sideways_pan_in_points
            + self.usable_width() * ((ns - self.start_s) as f32) / options.canvas_width_s
    }
}

const GUTTER_WIDTH: f32 = 200.0; // fixed left bar width for labels

fn site_from_fqdn(host: &str) -> Option<String> {
    let mut parts = host.split('.');
    let _hostname = parts.next();
    let site = parts.next();
    site.filter(|s| !s.is_empty()).map(|s| s.to_string())
}

fn site_for_cluster_name(cluster_name: &str, clusters: &[Cluster]) -> Option<String> {
    clusters
        .iter()
        .find(|c| c.name == cluster_name)
        .and_then(|c| c.hosts.first())
        .and_then(|h| site_from_fqdn(&h.network_address).or_else(|| site_from_fqdn(&h.name)))
}

/****************************************************************************************************************************/
// OPTIONS
/****************************************************************************************************************************/

/**
 * Options struct
 */
pub struct Options {
    pub canvas_width_s: f32,                                   // Canvas width
    pub sideways_pan_in_points: f32,                           // Sideways pan in points
    pub cull_width: f32,                                       // Culling width
    pub min_width: f32,                                        // Minimum width of a job
    pub rect_height: f32,                                      // Height of a job
    pub spacing: f32,                                          // Vertical spacing between jobs
    pub rounding: f32,                                         // Rounded corners
    pub aggregate_by: AggregateBy,                             // Aggregate by
    pub job_color: JobColor,                                   // Job color
    pub see_all_res: bool,                                     // See all resources
    pub current_hovered_job: Option<Job>,                      // Current hovered job
    pub previous_hovered_job: Option<Job>,                     // Previous hovered job
    pub current_hovered_resource_state: Option<ResourceState>, // Current hovered resource state
    pub squash_resources: bool,                                // Squash resources
    #[cfg_attr(feature = "serde", serde(skip))]
    zoom_to_relative_s_range: Option<(f64, (f64, f64))>, // Zoom to relative s range
}

/**
 * Default implementation for the Options struct
 */
impl Default for Options {
    fn default() -> Self {
        Self {
            canvas_width_s: 0.0,              // no zoom
            sideways_pan_in_points: 0.0,      // no pan
            cull_width: 0.0,                  // no culling
            min_width: 1.0,                   // minimum width of a job
            rect_height: 16.0,                // height of a job
            spacing: 5.0,                     // vertical spacing between jobs
            rounding: 4.0,                    // rounded corners
            aggregate_by: Default::default(), // aggregate by component
            job_color: Default::default(),    // job color component
            zoom_to_relative_s_range: None,   // no zooming by default
            current_hovered_job: None,        // no hovered job by default
            previous_hovered_job: None,       // no previous hovered job by default
            squash_resources: false,          // don't squash resources by default
            see_all_res: false,
            current_hovered_resource_state: None, // no hovered resource stae by default
        }
    }
}

/****************************************************************************************************************************/
// CANVAS PAINTING
/****************************************************************************************************************************/

/**
 * Paints the UI canvas
 */
fn ui_canvas(
    options: &mut Options,
    app: &ApplicationContext,
    info: &Info,
    fixed_timeline_y: f32,
    (min_ns, max_ns): (i64, i64),
    details_window: &mut Vec<JobDetailsWindow>,
    collapsed_jobs_level_1: &mut BTreeMap<String, bool>,
    collapsed_jobs_level_2: &mut BTreeMap<(String, String), bool>,
    all_cluster: &Vec<Cluster>,
    gutter_width: f32,
) -> f32 {
    if options.canvas_width_s <= 0.0 {
        options.canvas_width_s = (max_ns - min_ns) as f32;
        options.zoom_to_relative_s_range = None;
    }

    let mut cursor_y = info.canvas.top();
    cursor_y += info.text_height;

    // Paint a persistent gutter background (behind labels)
    let theme_colors = get_theme_colors(&info.ctx.style());
    let is_grid5000_gutter = options.aggregate_by.level_1 == AggregateByLevel1Enum::Cluster
        && options.aggregate_by.level_2 == AggregateByLevel2Enum::Host;
    let gutter_bg = if is_grid5000_gutter {
        if info.ctx.style().visuals.dark_mode {
            Color32::from_rgb(125, 115, 55)
        } else {
            Color32::from_rgb(245, 227, 113)
        }
    } else {
        theme_colors.background
    };
    let gutter_rect = Rect::from_min_max(
        pos2(info.canvas.min.x, info.canvas.min.y),
        pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
    );
    info.painter
        .rect_filled(gutter_rect, 0.0, gutter_bg);
    info.painter.line_segment(
        [
            pos2(info.canvas.min.x + gutter_width, info.canvas.min.y),
            pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
        ],
        Stroke::new(1.0, theme_colors.line),
    );

    // Get filtered jobs to display
    let jobs = app.filtered_jobs.clone();

    match options.aggregate_by.level_1 {

        // Aggregate by owner as level 1 only
        AggregateByLevel1Enum::Owner => {
            let mut jobs_by_owner: BTreeMap<String, Vec<Job>> = BTreeMap::new();
            // for each job, we add it to the corresponding owner
            for job in jobs {
                jobs_by_owner
                    .entry(job.owner.clone())
                    .or_insert_with(Vec::new)
                    .push(job.clone());
            }
            // Paint the aggregated jobs with owner as level 1
            cursor_y = paint_aggregated_jobs_level_1(
                info,
                options,
                jobs_by_owner,
                cursor_y,
                details_window,
                collapsed_jobs_level_1,
                app.font_size,
                all_cluster,
                AggregateByLevel1Enum::Owner,
                gutter_width,
            );
        }

        // Aggregate by host as level 1
        AggregateByLevel1Enum::Host => {
            match options.aggregate_by.level_2 {

                // Aggregate by owner as level 2
                AggregateByLevel2Enum::Owner => {
                    let mut jobs_by_host_by_owner: BTreeMap<String, BTreeMap<String, Vec<Job>>> =
                        BTreeMap::new();
                    let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();
                    // for each job, we add it to the corresponding host and owner
                    for job in jobs {
                        for host in job.hosts.iter() {
                            if filtered_clusters.len() != 0
                                && !contains_host(&filtered_clusters, host)
                            {
                                continue;
                            }
                            jobs_by_host_by_owner
                                .entry(host.clone())
                                .or_insert_with(BTreeMap::new)
                                .entry(job.owner.clone())
                                .or_insert_with(Vec::new)
                                .push(job.clone());
                        }
                    }

                    // Paint the aggregated jobs with host as level 1 and owner as level 2
                    cursor_y = paint_aggregated_jobs_level_2(
                        info,
                        options,
                        jobs_by_host_by_owner,
                        cursor_y,
                        details_window,
                        collapsed_jobs_level_1,
                        collapsed_jobs_level_2,
                        app.font_size,
                        all_cluster,
                        AggregateByLevel1Enum::Host,
                        AggregateByLevel2Enum::Owner,
                        gutter_width,
                    );
                }

                // No aggregation as level 2 so we only aggregate by host as level 1
                AggregateByLevel2Enum::None => {
                    let mut jobs_by_host: BTreeMap<String, Vec<Job>> = BTreeMap::new();
                    let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                    // for each job, we add it to the corresponding host
                    for job in jobs {
                        for host in job.hosts.iter() {
                            if filtered_clusters.len() != 0
                                && !contains_host(&filtered_clusters, host)
                            {
                                continue;
                            }
                            jobs_by_host
                                .entry(host.clone())
                                .or_insert_with(Vec::new)
                                .push(job.clone());
                        }
                    }

                    // Paint the aggregated jobs with host as level 1
                    cursor_y = paint_aggregated_jobs_level_1(
                        info,
                        options,
                        jobs_by_host,
                        cursor_y,
                        details_window,
                        collapsed_jobs_level_1,
                        app.font_size,
                        all_cluster,
                        AggregateByLevel1Enum::Host,
                        gutter_width,
                    );
                }
                AggregateByLevel2Enum::Host => {
                    // nothing to do here
                }
            }
        }

        // Aggregate by cluster as level 1
        AggregateByLevel1Enum::Cluster => match options.aggregate_by.level_2 {

            // Aggregate by owner as level 2
            AggregateByLevel2Enum::Owner => {
                let mut jobs_by_cluster_by_owner: BTreeMap<String, BTreeMap<String, Vec<Job>>> =
                    BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                // for each job, we add it to the corresponding cluster and owner
                for job in jobs {
                    for cluster in job.clusters.iter() {
                        if filtered_clusters.len() != 0
                            && contains_cluster(&filtered_clusters, cluster)
                        {
                            continue;
                        }
                        jobs_by_cluster_by_owner
                            .entry(cluster.clone())
                            .or_insert_with(BTreeMap::new)
                            .entry(job.owner.clone())
                            .or_insert_with(Vec::new)
                            .push(job.clone());
                    }
                }

                // Paint the aggregated jobs with cluster as level 1 and owner as level 2
                cursor_y = paint_aggregated_jobs_level_2(
                    info,
                    options,
                    jobs_by_cluster_by_owner,
                    cursor_y,
                    details_window,
                    collapsed_jobs_level_1,
                    collapsed_jobs_level_2,
                    app.font_size,
                    all_cluster,
                    AggregateByLevel1Enum::Cluster,
                    AggregateByLevel2Enum::Owner,
                    gutter_width,
                );
            }

            // No aggregation as level 2 so we only aggregate by cluster as level 1
            AggregateByLevel2Enum::None => {
                let mut jobs_by_cluster: BTreeMap<String, Vec<Job>> = BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                // for each job, we add it to the corresponding cluster
                for job in jobs {
                    for cluster in job.clusters.iter() {
                        if filtered_clusters.len() != 0
                            && contains_cluster(&filtered_clusters, cluster)
                        {
                            continue;
                        }
                        jobs_by_cluster
                            .entry(cluster.clone())
                            .or_insert_with(Vec::new)
                            .push(job.clone());
                    }
                }

                // Paint the aggregated jobs with cluster as level 1
                cursor_y = paint_aggregated_jobs_level_1(
                    info,
                    options,
                    jobs_by_cluster,
                    cursor_y,
                    details_window,
                    collapsed_jobs_level_1,
                    app.font_size,
                    all_cluster,
                    AggregateByLevel1Enum::Cluster,
                    gutter_width,
                );
            }

            // Aggregate by host as level 2
            AggregateByLevel2Enum::Host => {
                let mut jobs_by_cluster_by_host: BTreeMap<String, BTreeMap<String, Vec<Job>>> =
                    BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                // for each job, we add it to the corresponding cluster and host
                for job in jobs {
                    for cluster in job.clusters.iter() {
                        for host in job.hosts.iter() {
                            if filtered_clusters.len() != 0
                                && !contains_host(&filtered_clusters, host)
                            {
                                continue;
                            }
                            // We don't add the host to the cluster if this host doesn't belong to the cluster
                            let curr_cluster =
                                get_cluster_from_name(&app.all_clusters, &cluster).unwrap();

                            if cluster_contain_host(&curr_cluster, &host) {
                                jobs_by_cluster_by_host
                                    .entry(cluster.clone())
                                    .or_insert_with(BTreeMap::new)
                                    .entry(host.clone())
                                    .or_insert_with(Vec::new)
                                    .push(job.clone());
                            }
                        }
                    }
                }

                // Paint the aggregated jobs with cluster as level 1 and host as level 2
                cursor_y = paint_aggregated_jobs_level_2(
                    info,
                    options,
                    jobs_by_cluster_by_host,
                    cursor_y,
                    details_window,
                    collapsed_jobs_level_1,
                    collapsed_jobs_level_2,
                    app.font_size,
                    all_cluster,
                    AggregateByLevel1Enum::Cluster,
                    AggregateByLevel2Enum::Host,
                    gutter_width,
                );
            }
        },
    }

    // Paint tooltip for hovered job/resource state
    paint_tooltip(info, options);

    // Update previous hovered job for next frame
    options.previous_hovered_job = options.current_hovered_job.clone();
    options.current_hovered_job = None; // Reset for next frame, but keep previous_hovered_job

    // Paint the timeline text on top of everything
    paint_timeline_text_on_top(info, options, fixed_timeline_y, gutter_width);

    cursor_y
}

/**
 * Interacts with the canvas
 */
fn interact_with_canvas(options: &mut Options, response: &Response, info: &Info) {
    if response.drag_delta().x != 0.0 {
        options.sideways_pan_in_points += response.drag_delta().x;
        options.zoom_to_relative_s_range = None;
    }

    if response.hovered() {
        // Sideways pan with e.g. a touch pad:
        if info.ctx.input(|i| i.smooth_scroll_delta.x != 0.0) {
            options.sideways_pan_in_points += info.ctx.input(|i| i.smooth_scroll_delta.x);
            options.zoom_to_relative_s_range = None;
        }

        let mut zoom_factor = info.ctx.input(|i| i.zoom_delta_2d().x);

        if response.dragged_by(PointerButton::Secondary) {
            zoom_factor *= (response.drag_delta().y * 0.01).exp();
        }

        if zoom_factor != 1.0 {
            let new_width = options.canvas_width_s / zoom_factor;

            // Apply a limit to the zoom
            let max_canvas_width = 2 * 24 * 60 * 60; // 2 days in seconds
            if new_width <= max_canvas_width as f32 {
                options.canvas_width_s = new_width;

                if let Some(mouse_pos) = response.hover_pos() {
                    let origin_x = info.canvas.min.x + info.gutter_width;
                    let zoom_center = mouse_pos.x - origin_x;
                    options.sideways_pan_in_points =
                        (options.sideways_pan_in_points - zoom_center) * zoom_factor + zoom_center;
                }
            }
            options.zoom_to_relative_s_range = None;
        }
    }

    if response.double_clicked() {
        // Reset view
        options.zoom_to_relative_s_range = Some((
            info.ctx.input(|i| i.time),
            (0., (info.stop_s - info.start_s) as f64),
        ));
    }

    if let Some((start_time, (start_s, end_s))) = options.zoom_to_relative_s_range {
        const ZOOM_DURATION: f32 = 0.75;
        let t = (info.ctx.input(|i| i.time - start_time) as f32 / ZOOM_DURATION).min(1.0);

        let canvas_width = info.usable_width();

        let target_canvas_width_s = (end_s - start_s) as f32;
        let target_pan_in_points = -canvas_width * start_s as f32 / target_canvas_width_s;

        options.canvas_width_s = lerp(
            options.canvas_width_s.recip()..=target_canvas_width_s.recip(),
            t,
        )
        .recip();
        options.sideways_pan_in_points =
            lerp(options.sideways_pan_in_points..=target_pan_in_points, t);

        if t >= 1.0 {
            options.zoom_to_relative_s_range = None;
        }

        info.ctx.request_repaint();
    }
}

/**
 * ThemeColors struct
 */
struct ThemeColors {
    text: Color32,
    text_dim: Color32,
    line: Color32,
    aggregated_line_level_1: Rgba,
    aggregated_line_level_2: Rgba,
    background: Color32,
    background_timeline: Color32,
    hatch: Color32,
}

/**
 * Returns the theme colors for the Gantt chart
 */
fn get_theme_colors(style: &egui::Style) -> ThemeColors {

    if style.visuals.dark_mode { // Dark mode
        ThemeColors {
            text: Color32::WHITE,
            text_dim: Color32::from_white_alpha(229),
            line: Color32::WHITE,
            aggregated_line_level_1: Rgba::from_white_alpha(0.4),
            aggregated_line_level_2: Rgba::from_white_alpha(0.5),
            background: Color32::from_black_alpha(100),
            background_timeline: Color32::from_black_alpha(150),
            hatch: Color32::from_rgba_premultiplied(0, 150, 150, 150),
        }
    } else { // Light mode
        ThemeColors {
            text: Color32::BLACK,
            text_dim: Color32::from_black_alpha(229),
            line: Color32::BLACK,
            aggregated_line_level_1: Rgba::from_black_alpha(0.5),
            aggregated_line_level_2: Rgba::from_black_alpha(0.7),
            background: Color32::from_black_alpha(50),
            background_timeline: Color32::from_black_alpha(20),
            hatch: Color32::from_rgba_premultiplied(0, 0, 139, 150),
        }
    }
}

struct LabelMeta {
    host: Option<String>,
}

fn short_host_label(host: &str) -> String {
    let first = host.split('.').next().unwrap_or(host).trim();

    // If it's like "dahu-1" => "dahu1" (matches requested example)
    if let Some(idx) = first.rfind('-') {
        let (left, right_with_dash) = first.split_at(idx);
        let right = right_with_dash.trim_start_matches('-');
        if !left.is_empty()
            && !right.is_empty()
            && left.chars().all(|c| c.is_ascii_alphanumeric())
            && right.chars().all(|c| c.is_ascii_digit())
        {
            return format!("{}{}", left, right);
        }
    }

    first.to_string()
}

fn build_label_meta_level1(
    level_1: &str,
    aggregate_by_level_1: AggregateByLevel1Enum,
    clusters: &[Cluster],
) -> Option<LabelMeta> {
    match aggregate_by_level_1 {
        AggregateByLevel1Enum::Cluster => {
            let _ = clusters;
            None
        }
        AggregateByLevel1Enum::Host => {
            Some(LabelMeta {
                host: Some(level_1.to_string()),
            })
        }
        AggregateByLevel1Enum::Owner => None,
    }
}

fn build_label_meta_level2(
    level_1: &str,
    level_2: &str,
    aggregate_by_level_1: AggregateByLevel1Enum,
    aggregate_by_level_2: AggregateByLevel2Enum,
    clusters: &[Cluster],
) -> Option<LabelMeta> {
    match aggregate_by_level_2 {
        AggregateByLevel2Enum::Host => {
            let _ = (level_1, aggregate_by_level_1, clusters);
            Some(LabelMeta {
                host: Some(level_2.to_string()),
            })
        }
        _ => None,
    }
}

/****************************************************************************************************************************/
// JOB PAINTING
/****************************************************************************************************************************/

/**
 * Paints a tooltip for a job and/or resource state
 */
fn paint_tooltip(info: &Info, options: &mut Options) {
    let mut tooltip_text = String::new();

    // Add job info if there's a hovered job
    if let Some(job) = &options.current_hovered_job {
        tooltip_text.push_str(&format!(
            "{}: {}\n{}: {:?}\n{}: {}\n{}: {}\n{}: {} seconds",
            t!("app.details.tooltip.job_id"),
            job.id,
            t!("app.details.tooltip.owner"),
            job.owner,
            t!("app.details.tooltip.state"),
            job.state.get_label(),
            t!("app.details.tooltip.start_time"),
            format_timestamp(job.scheduled_start),
            t!("app.details.tooltip.walltime"),
            job.walltime
        ));
    }

    // Add resource state info if there's a hovered resource state
    if let Some(resource_state) = &options.current_hovered_resource_state {
        if !tooltip_text.is_empty() {
            tooltip_text.push_str("\n");
        }
        tooltip_text.push_str(&format!(
            "{} State: {:?}",
            if (options.aggregate_by.level_2 == AggregateByLevel2Enum::None
                && options.aggregate_by.level_1 == AggregateByLevel1Enum::Host)
                || options.aggregate_by.level_2 == AggregateByLevel2Enum::Host
            {
                "Host"
            } else if options.aggregate_by.level_2 == AggregateByLevel2Enum::None
                && options.aggregate_by.level_1 == AggregateByLevel1Enum::Cluster
            {
                "Cluster"
            } else {
                "Resource"
            },
            resource_state
        ));
        options.current_hovered_resource_state = None; // Reset for next frame
    }

    // Show tooltip if we have any text to display
    if !tooltip_text.is_empty() {
        if let Some(_pointer_pos) = info.response.hover_pos() {
            egui::show_tooltip_at_pointer(
                &info.ctx,
                info.response.layer_id,
                egui::Id::new("tooltip"),
                |ui| {
                    ui.set_max_width(800.0);
                    ui.label(tooltip_text);
                },
            );
        }
    }
}

/**
 * Paints jobs with 1 level of aggregation
 */
fn paint_aggregated_jobs_level_1(
    info: &Info,
    options: &mut Options,
    jobs: BTreeMap<String, Vec<Job>>,
    mut cursor_y: f32,
    details_window: &mut Vec<JobDetailsWindow>,
    collapsed_jobs: &mut BTreeMap<String, bool>,
    font_size: i32,
    all_cluster: &Vec<Cluster>,
    aggregate_by: AggregateByLevel1Enum,
    gutter_width: f32,
) -> f32 {
    let theme_colors = get_theme_colors(&info.ctx.style());

    let spacing_between_level_1 = font_size as f32 * 0.25; // minimal spacing
    let spacing_between_jobs = 0.0; // no gap between jobs within owner
    let offset_level_1 = 6.0; // compact header offset

    cursor_y += spacing_between_level_1;

    // Sort the level 1 keys
    let mut sorted_level_1: Vec<String> = jobs.keys().cloned().collect();
    sorted_level_1.sort_by(|a, b| compare_string_with_number(&a, &b));

    let aggregation_height = font_size as f32 + 5.0 + offset_level_1;

    // Store header positions and data to draw them last if squashing is enabled
    let mut header_data: Vec<(String, Pos2, bool, Option<LabelMeta>)> = Vec::new();

    // Display jobs
    for level_1 in sorted_level_1 {
        let job_list = jobs.get(&level_1).unwrap();
        // Draw a line to separate
        info.painter.line_segment(
            [
                pos2(info.canvas.min.x, cursor_y),
                pos2(info.canvas.max.x, cursor_y),
            ],
            Stroke::new(1.5, theme_colors.aggregated_line_level_1), // More marked line
        );

        cursor_y += offset_level_1;

        let text_pos = pos2(info.canvas.min.x, cursor_y);

        // Check if the section is collapsed
        let is_collapsed = collapsed_jobs.entry(level_1.clone()).or_insert(false);
        *is_collapsed = false; // keep rows expanded; no collapsing like reference
        let label_meta = build_label_meta_level1(&level_1, aggregate_by, all_cluster);

        // Store header info for later if squashing
        if options.squash_resources {
            header_data.push((level_1.clone(), text_pos, *is_collapsed, label_meta));
        } else {
            // Paint the job info immediately if not squashing
            paint_job_info(
                info,
                &level_1,
                text_pos,
                is_collapsed,
                1,
                gutter_width,
                options.rect_height,
                label_meta,
            );
        }

        cursor_y += spacing_between_level_1; // Spacing after the owner

        let state;

        // Get the state of the resource
        if aggregate_by == AggregateByLevel1Enum::Owner {
            state = ResourceState::Alive;
        } else if aggregate_by == AggregateByLevel1Enum::Host {
            state = get_host_state_from_name(all_cluster, &level_1);
        } else {
            state = get_cluster_state_from_name(all_cluster, &level_1);
        }

        // Only show jobs if section is not collapsed
        if !*is_collapsed {
            // Save the initial cursor position for squashed rendering
            let initial_job_y = cursor_y;

            for job in job_list {
                // When squashing, use the initial Y position for all jobs in this level
                let job_start_y = if options.squash_resources {
                    initial_job_y
                } else {
                    cursor_y
                };

                // Draw the job
                paint_job(
                    info,
                    options,
                    &job,
                    job_start_y,
                    details_window,
                    all_cluster,
                    state,
                    aggregation_height,
                );

                // Only increment cursor if not squashing
                if !options.squash_resources {
                    cursor_y += info.text_height + spacing_between_jobs + options.spacing;
                }
            }

            // If we're squashing, just add one line of spacing after all jobs
            if options.squash_resources && !job_list.is_empty() {
                cursor_y += info.text_height + spacing_between_jobs + options.spacing;
            }
            if !options.squash_resources {
                cursor_y += spacing_between_level_1;
            }
        }
        if !options.squash_resources {
            cursor_y += spacing_between_level_1;
        }
    }

    // Now draw all headers on top if squashing is enabled
    if options.squash_resources {
        for (name, pos, is_collapsed, label_meta) in header_data {
            // Create a background to make the text more readable
            let galley = info.ctx.fonts(|f| {
                let collapsed_symbol = if is_collapsed { "⏵" } else { "⏷" };
                let label = format!("{} {}", collapsed_symbol, name);
                f.layout_no_wrap(label, info.font_id.clone(), theme_colors.text_dim)
            });
            let rect = Rect::from_min_size(pos, galley.size());
            info.painter
                .rect_filled(rect.expand(4.0), 4.0, theme_colors.background_timeline);

            // Then paint the job info
            let mut is_collapsed_copy = is_collapsed;
            paint_job_info(
                info,
                &name,
                pos,
                &mut is_collapsed_copy,
                1,
                gutter_width,
                options.rect_height,
                label_meta,
            );
            // Update the real collapsed state if changed
            if is_collapsed_copy != is_collapsed {
                *collapsed_jobs.get_mut(&name).unwrap() = is_collapsed_copy;
            }
        }
    }

    cursor_y
}

/**
 * Paints jobs with 2 levels of aggregation
 */
fn paint_aggregated_jobs_level_2(
    info: &Info,
    options: &mut Options,
    jobs: BTreeMap<String, BTreeMap<String, Vec<Job>>>,
    mut cursor_y: f32,
    details_window: &mut Vec<JobDetailsWindow>,
    collapsed_jobs_level_1: &mut BTreeMap<String, bool>,
    collapsed_jobs_level_2: &mut BTreeMap<(String, String), bool>,
    font_size: i32,
    all_cluster: &Vec<Cluster>,
    aggregate_by_level_1: AggregateByLevel1Enum,
    aggregate_by_level_2: AggregateByLevel2Enum,
    gutter_width: f32,
) -> f32 {
    let theme_colors = get_theme_colors(&info.ctx.style());

    // Grid5000-like view: when aggregating Cluster -> Host, only show hosts in the gutter.
    let hide_level_1_headers = aggregate_by_level_1 == AggregateByLevel1Enum::Cluster
        && aggregate_by_level_2 == AggregateByLevel2Enum::Host;

    #[derive(Clone)]
    struct GanttGutterHostRow {
        host: String,
        row_rect: Rect,
    }

    #[derive(Clone)]
    struct GanttGutterSpan {
        label: String,
        rect: Rect,
    }

    let mut grid5000_host_rows: Vec<GanttGutterHostRow> = Vec::new();
    let mut grid5000_cluster_spans: Vec<GanttGutterSpan> = Vec::new();
    let mut grid5000_site_spans: Vec<GanttGutterSpan> = Vec::new();

    let mut current_site: Option<(String, f32, f32)> = None; // (label, top, bottom)

    let spacing_between_level_1 = font_size as f32 * 0.25; // minimal spacing between clusters
    let spacing_between_level_2 = font_size as f32 * 0.35; // minimal spacing between hosts
    let spacing_between_jobs = 0.0; // no gap between jobs
    let offset_level_1 = 6.0; // compact offset for cluster header

    cursor_y += spacing_between_level_1;

    // Sort the level 1 keys
    let mut sorted_level_1: Vec<String> = jobs.keys().cloned().collect();
    sorted_level_1.sort_by(|a, b| compare_string_with_number(&a, &b));

    // Store header positions and data to draw them last if squashing is enabled
    let mut header_data_level_1: Vec<(String, Pos2, bool, Option<LabelMeta>)> = Vec::new();
    let mut header_data_level_2: Vec<(String, String, Pos2, bool, Option<LabelMeta>)> = Vec::new();

    for level_1 in sorted_level_1 {
        let level_2_map = jobs.get(&level_1).unwrap();
        let level_1_key = level_1.clone();

        let cluster_site = if hide_level_1_headers {
            site_for_cluster_name(&level_1, all_cluster).unwrap_or_default()
        } else {
            String::new()
        };

        let mut cluster_top: Option<f32> = None;
        let mut cluster_bottom: Option<f32> = None;

        if !hide_level_1_headers {
            // Draw a line to separate
            info.painter.line_segment(
                [
                    pos2(info.canvas.min.x, cursor_y),
                    pos2(info.canvas.max.x, cursor_y),
                ],
                Stroke::new(1.5, theme_colors.aggregated_line_level_1),
            );

            cursor_y += offset_level_1;

            let text_pos = pos2(info.canvas.min.x, cursor_y);

            // Check if the level 1 is collapsed
            let is_collapsed_level_1 = collapsed_jobs_level_1
                .entry(level_1.clone())
                .or_insert(false);
            *is_collapsed_level_1 = false; // force expanded
            let label_meta_level_1 =
                build_label_meta_level1(&level_1, aggregate_by_level_1, all_cluster);

            // Either store header info for later or paint immediately
            if options.squash_resources {
                header_data_level_1.push((
                    level_1.clone(),
                    text_pos,
                    *is_collapsed_level_1,
                    label_meta_level_1,
                ));
            } else {
                // Paint the job info immediately if not squashing
                paint_job_info(
                    info,
                    &level_1,
                    text_pos,
                    is_collapsed_level_1,
                    1,
                    gutter_width,
                    options.rect_height,
                    label_meta_level_1,
                );
            }

            cursor_y += spacing_between_level_1;
        } else {
            // Minimal spacing between clusters when cluster headers are hidden.
            cursor_y += spacing_between_level_1;
        }

        // Only show jobs if section is not collapsed (or always when headers are hidden)
        let is_collapsed_level_1 = collapsed_jobs_level_1
            .entry(level_1.clone())
            .or_insert(false);
        *is_collapsed_level_1 = false;

        if !*is_collapsed_level_1 {
            // Sort the level 2 keys
            let mut sorted_level_2: Vec<_> = level_2_map.keys().collect();
            sorted_level_2.sort_by(|a, b| compare_string_with_number(&a, &b));

            // Display level 2
            for level_2 in sorted_level_2 {
                if let Some(job_list) = level_2_map.get(level_2) {
                    // Draw a line to separate
                    info.painter.line_segment(
                        [
                            pos2(info.canvas.min.x, cursor_y),
                            pos2(info.canvas.max.x, cursor_y),
                        ],
                        Stroke::new(0.5, theme_colors.aggregated_line_level_2), // Line more discreet
                    );

                    cursor_y += spacing_between_level_2;

                    // Align labels to the same center as job rows (fixes scroll/misalignment artifacts)
                    let row_center_y = cursor_y + spacing_between_level_2;

                    let text_pos = pos2(info.canvas.min.x + 20.0, row_center_y);

                    // Check if the level 2 is collapsed
                    let is_collapsed_level_2 = collapsed_jobs_level_2
                        .entry((level_1_key.to_string(), level_2.to_string()))
                        .or_insert(false);
                    *is_collapsed_level_2 = false; // force expanded
                    let label_meta_level_2 = build_label_meta_level2(
                        &level_1,
                        level_2,
                        aggregate_by_level_1,
                        aggregate_by_level_2,
                        all_cluster,
                    );

                    // Either store header info for later or paint immediately
                    if options.squash_resources {
                        header_data_level_2.push((
                            level_1_key.clone(),
                            level_2.to_string(),
                            text_pos,
                            *is_collapsed_level_2,
                            label_meta_level_2,
                        ));
                    } else {
                        if hide_level_1_headers {
                            // Grid5000-style gutter rows for hosts
                            let row_height = options.rect_height.max(info.text_height + 10.0);
                            let row_rect = Rect::from_min_max(
                                pos2(info.canvas.min.x, row_center_y - row_height * 0.5),
                                pos2(info.canvas.min.x + gutter_width, row_center_y + row_height * 0.5),
                            );

                            let host_short = short_host_label(&level_2.to_string());
                            grid5000_host_rows.push(GanttGutterHostRow {
                                host: host_short,
                                row_rect,
                            });

                            cluster_top = Some(cluster_top.unwrap_or(row_rect.min.y).min(row_rect.min.y));
                            cluster_bottom = Some(cluster_bottom.unwrap_or(row_rect.max.y).max(row_rect.max.y));
                        } else {
                            // Paint the job info immediately if not squashing
                            paint_job_info(
                                info,
                                &level_2.to_string(),
                                text_pos,
                                is_collapsed_level_2,
                                2,
                                gutter_width,
                                options.rect_height,
                                label_meta_level_2,
                            );
                        }
                    }

                    cursor_y += spacing_between_level_2;

                    let state;

                    // Get the state of the resource
                    if aggregate_by_level_2 == AggregateByLevel2Enum::Host {
                        state = get_host_state_from_name(all_cluster, &level_2);
                    } else if aggregate_by_level_2 == AggregateByLevel2Enum::None {
                        if aggregate_by_level_1 == AggregateByLevel1Enum::Host {
                            state = get_host_state_from_name(all_cluster, &level_1);
                        } else if aggregate_by_level_1 == AggregateByLevel1Enum::Cluster {
                            state = get_cluster_state_from_name(all_cluster, &level_1);
                        } else {
                            state = ResourceState::Alive;
                        }
                    } else {
                        state = ResourceState::Alive;
                    }

                    // Only show jobs if section is not collapsed
                    if !*is_collapsed_level_2 {
                        // Save the initial cursor position for squashed rendering
                        let initial_job_y = cursor_y;

                        // Display jobs
                        for job in job_list.iter() {
                            // Align jobs vertically with the host bar
                            let aligned_y = cursor_y - options.rect_height * 0.5;
                            let job_start_y = if options.squash_resources {
                                initial_job_y
                            } else {
                                aligned_y
                            };

                            // Adjust aggregation height for the first job of the second level
                            let adjusted_aggregation_height = spacing_between_level_2 * 2.0;

                            // Draw the job
                            paint_job(
                                info,
                                options,
                                job,
                                job_start_y,
                                details_window,
                                all_cluster,
                                state,
                                adjusted_aggregation_height,
                            );

                            // Only increment cursor if not squashing
                            if !options.squash_resources && !job_list.is_empty() {
                                cursor_y += info.text_height + spacing_between_jobs;
                            }
                        }

                        // If we're squashing, just add one line of spacing after all jobs
                        if options.squash_resources {
                            cursor_y += info.text_height + spacing_between_jobs;
                        }
                    }
                    if !options.squash_resources {
                        cursor_y += spacing_between_level_2;
                    }
                }
            }
        }
        if !options.squash_resources {
            cursor_y += spacing_between_level_1;
        }

        // Finalize Grid5000-style spans for this cluster
        if hide_level_1_headers {
            if let (Some(top), Some(bottom)) = (cluster_top, cluster_bottom) {
                // Cluster span
                let site_w = (gutter_width * 0.28).clamp(40.0, 90.0);
                let cluster_w = (gutter_width * 0.36).clamp(60.0, 120.0);

                grid5000_cluster_spans.push(GanttGutterSpan {
                    label: level_1.clone(),
                    rect: Rect::from_min_max(
                        pos2(info.canvas.min.x + site_w, top),
                        pos2(info.canvas.min.x + site_w + cluster_w, bottom),
                    ),
                });

                // Site span (may cover multiple clusters)
                if let Some((label, s_top, s_bottom)) = current_site.as_mut() {
                    if *label == cluster_site {
                        *s_bottom = (*s_bottom).max(bottom);
                    } else {
                        grid5000_site_spans.push(GanttGutterSpan {
                            label: label.clone(),
                            rect: Rect::from_min_max(
                                pos2(info.canvas.min.x, *s_top),
                                pos2(info.canvas.min.x + site_w, *s_bottom),
                            ),
                        });
                        *label = cluster_site.clone();
                        *s_top = top;
                        *s_bottom = bottom;
                    }
                } else {
                    current_site = Some((cluster_site.clone(), top, bottom));
                }
            }
        }
    }

    // Close last site span
    if hide_level_1_headers {
        if let Some((label, top, bottom)) = current_site.take() {
            let site_w = (gutter_width * 0.28).clamp(40.0, 90.0);
            grid5000_site_spans.push(GanttGutterSpan {
                label,
                rect: Rect::from_min_max(
                    pos2(info.canvas.min.x, top),
                    pos2(info.canvas.min.x + site_w, bottom),
                ),
            });
        }
    }

    // Paint Grid5000-style gutter overlays (site/cluster/host columns) on top.
    if hide_level_1_headers {
        let gutter_clip = Rect::from_min_max(
            info.canvas.min,
            pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
        );
        let gutter_painter = info.painter.with_clip_rect(gutter_clip);

        let site_w = (gutter_width * 0.28).clamp(40.0, 90.0);
        let cluster_w = (gutter_width * 0.36).clamp(60.0, 120.0);
        let host_w = (gutter_width - site_w - cluster_w).max(60.0);

        let (c_site, c_cluster, c_host, c_border, c_text) = if info.ctx.style().visuals.dark_mode {
            (
                Color32::from_rgb(150, 140, 70),
                Color32::from_rgb(165, 155, 80),
                Color32::from_rgb(180, 170, 95),
                Color32::from_gray(30),
                Color32::from_gray(10),
            )
        } else {
            (
                Color32::from_rgb(235, 215, 110),
                Color32::from_rgb(245, 227, 113),
                Color32::from_rgb(252, 238, 170),
                Color32::from_gray(30),
                Color32::BLACK,
            )
        };

        // Column separators
        gutter_painter.line_segment(
            [
                pos2(info.canvas.min.x + site_w, info.canvas.min.y),
                pos2(info.canvas.min.x + site_w, info.canvas.max.y),
            ],
            Stroke::new(1.0, c_border),
        );
        gutter_painter.line_segment(
            [
                pos2(info.canvas.min.x + site_w + cluster_w, info.canvas.min.y),
                pos2(info.canvas.min.x + site_w + cluster_w, info.canvas.max.y),
            ],
            Stroke::new(1.0, c_border),
        );

        let font_site = FontId::proportional((info.font_id.size + 1.0).max(12.0));
        let font_cluster = FontId::proportional((info.font_id.size + 1.0).max(12.0));
        let font_host = FontId::proportional((info.font_id.size).max(11.0));

        // Site blocks
        for span in &grid5000_site_spans {
            gutter_painter.rect_filled(span.rect, 0.0, c_site);
            gutter_painter.rect(span.rect, 0.0, c_site, Stroke::new(1.0, c_border));
            let clip = gutter_painter.with_clip_rect(span.rect);
            clip.text(
                pos2(span.rect.center().x, span.rect.center().y),
                Align2::CENTER_CENTER,
                &span.label,
                font_site.clone(),
                c_text,
            );
        }

        // Cluster blocks
        for span in &grid5000_cluster_spans {
            gutter_painter.rect_filled(span.rect, 0.0, c_cluster);
            gutter_painter.rect(span.rect, 0.0, c_cluster, Stroke::new(1.0, c_border));
            let clip = gutter_painter.with_clip_rect(span.rect);
            clip.text(
                pos2(span.rect.center().x, span.rect.center().y),
                Align2::CENTER_CENTER,
                &span.label,
                font_cluster.clone(),
                c_text,
            );
        }

        // Host rows
        for row in &grid5000_host_rows {
            let host_rect = Rect::from_min_max(
                pos2(info.canvas.min.x + site_w + cluster_w, row.row_rect.min.y),
                pos2(info.canvas.min.x + site_w + cluster_w + host_w, row.row_rect.max.y),
            );
            gutter_painter.rect_filled(host_rect, 0.0, c_host);
            gutter_painter.rect(host_rect, 0.0, c_host, Stroke::new(1.0, c_border));
            let clip = gutter_painter.with_clip_rect(host_rect);
            clip.text(
                pos2(host_rect.min.x + 6.0, host_rect.center().y),
                Align2::LEFT_CENTER,
                &row.host,
                font_host.clone(),
                c_text,
            );
        }
    }

    // Now draw all headers on top if squashing is enabled
    if options.squash_resources {
        // First, draw level 1 headers (unless hidden)
        if !hide_level_1_headers {
            for (name, pos, is_collapsed, label_meta) in header_data_level_1 {
                // Create a background to make the text more readable
                let galley = info.ctx.fonts(|f| {
                    let collapsed_symbol = if is_collapsed { "⏵" } else { "⏷" };
                    let label = format!("{} {}", collapsed_symbol, name);
                    f.layout_no_wrap(label, info.font_id.clone(), theme_colors.text_dim)
                });
                let rect = Rect::from_min_size(pos, galley.size());
                info.painter
                    .rect_filled(rect.expand(4.0), 4.0, theme_colors.background_timeline);

                // Then paint the job info
                let mut is_collapsed_copy = is_collapsed;
                paint_job_info(
                    info,
                    &name,
                    pos,
                    &mut is_collapsed_copy,
                    1,
                    gutter_width,
                    options.rect_height,
                    label_meta,
                );
                // Update the real collapsed state if changed
                if is_collapsed_copy != is_collapsed {
                    *collapsed_jobs_level_1.get_mut(&name).unwrap() = is_collapsed_copy;
                }
            }
        }

        // Then draw level 2 headers on top of everything
        for (level_1_key, level_2_key, pos, is_collapsed, label_meta) in header_data_level_2 {
            // Then paint the job info
            let mut is_collapsed_copy = is_collapsed;
            paint_job_info(
                info,
                &level_2_key,
                pos,
                &mut is_collapsed_copy,
                2,
                gutter_width,
                options.rect_height,
                label_meta,
            );
            // Update the real collapsed state if changed
            if is_collapsed_copy != is_collapsed {
                *collapsed_jobs_level_2
                    .get_mut(&(level_1_key, level_2_key))
                    .unwrap() = is_collapsed_copy;
            }
        }
    }

    cursor_y
}


#[derive(PartialEq)]
enum PaintResult {
    Culled,
    Painted,
    Hovered,
}

/**
 * Paints a job
 */
fn paint_job(
    info: &Info,
    options: &mut Options,
    job: &Job,
    top_y: f32,
    details_window: &mut Vec<JobDetailsWindow>,
    all_cluster: &Vec<Cluster>,
    state: ResourceState,
    aggregation_height: f32,
) -> PaintResult {
    let theme_colors = get_theme_colors(&info.ctx.style());
    let chart_clip_rect = Rect::from_min_max(
        pos2(info.canvas.min.x + info.gutter_width, info.canvas.min.y),
        pos2(info.canvas.max.x, info.canvas.max.y),
    );
    let chart_painter = info.painter.with_clip_rect(chart_clip_rect);
    let start_x = info.point_from_s(options, job.scheduled_start); // Start time
    // If the job is not finished, we use the scheduled start time + walltime otherwise we use the stop time
    let stop_time = if job.stop_time > 0 {
        job.stop_time
    } else {
        job.scheduled_start + job.walltime
    };
    let end_x = info.point_from_s(options, stop_time);
    let width = end_x - start_x; // Width of the job

    if width < options.cull_width {
        return PaintResult::Culled;
    }

    // Calculate the total line height including all spacing components
    let spacing_between_jobs = 5.0;
    let total_line_height = info.text_height + spacing_between_jobs + options.spacing;

    // When squashing, use the full line height to completely eliminate gaps
    let height = if options.squash_resources {
        total_line_height + aggregation_height
    } else {
        options.rect_height
    };

    // Use no rounding when squashing
    let rounding = if options.squash_resources {
        0.0
    } else {
        options.rounding
    };

    // Create the rectangle for the job
    let rect = Rect::from_min_size(
        pos2(
            start_x,
            if options.squash_resources {
                top_y - aggregation_height
            } else {
                top_y
            },
        ),
        egui::vec2(width.max(options.min_width), height),
    );

    // Clip jobs strictly to the chart area (never into the left gutter).
    let visible_rect = rect.intersect(chart_clip_rect);
    if visible_rect.is_negative() {
        return PaintResult::Culled;
    }

    let is_job_trully_hovered = if let Some(mouse_pos) = info.response.hover_pos() {
        visible_rect.contains(mouse_pos)
    } else {
        false
    };

    // Update how we determine if a job is hovered
    let is_job_hovered = (if let Some(mouse_pos) = info.response.hover_pos() {
        visible_rect.contains(mouse_pos)
    } else {
        false
    }) || options
        .current_hovered_job
        .as_ref()
        .map_or(false, |j| j.id == job.id)
        || options
            .previous_hovered_job
            .as_ref()
            .map_or(false, |j| j.id == job.id);

    // If this job is being directly hovered, update the tooltip and hovered ID
    if is_job_trully_hovered && options.current_hovered_job.is_none() {
        options.current_hovered_job = Some(job.clone());
    }

    // Add click detection for the job
    if is_job_hovered && info.response.secondary_clicked() {
        let window =
            JobDetailsWindow::new(job.clone(), get_tree_structure_for_job(job, all_cluster));
        // Check if a window for this job already exists, if so, don't open a new one
        if !details_window.iter().any(|w| w.job.id == job.id) {
            details_window.push(window);
        }
    }

    // Zoom to job if clicked
    if is_job_hovered && info.response.clicked() {
        // Zoom to job
        let job_duration_s = job.walltime as f64;
        let job_start_s = job.scheduled_start as f64;
        let job_end_s = if job.stop_time > 0 {
            job.stop_time as f64
        } else {
            job_start_s + job_duration_s
        };
        options.zoom_to_relative_s_range = Some((
            info.ctx.input(|i| i.time),
            (
                job_start_s - info.start_s as f64,
                job_end_s - info.start_s as f64,
            ),
        ));
    }

    // Caculate the color of the job depending the selected job color int gantt component
    let (hovered_color, normal_color) = if options.job_color.is_random() {
        job.get_gantt_color()
    } else {
        job.state.get_color()
    };

    // If the job is hovered, we make it brighter
    let fill_color = if is_job_hovered {
        hovered_color
    } else {
        normal_color
    };

    // Paint the job rectangle
    chart_painter.rect_filled(visible_rect, rounding, fill_color);

    // Paint ressource hatch
    if state == ResourceState::Dead || state == ResourceState::Absent {

        // Define the color of the hachure depending the state of the resource
        let hachure_color = match state {
            ResourceState::Dead => Color32::from_rgba_premultiplied(255, 0, 0, 150),
            ResourceState::Absent => theme_colors.hatch,
            _ => Color32::TRANSPARENT,
        };

        let hachure_spacing = 10.0; // Spacing between hachure lines
        let mut shapes = Vec::new();
        let mut x = info.canvas.min.x;
        let current_time_x = info.point_from_s(options, chrono::Utc::now().timestamp());

        // Use the same y-position adjustment as the job rectangle
        let hatch_y = if options.squash_resources {
            top_y - aggregation_height
        } else {
            top_y
        };

        // Define the rectangle where the hachure will be drawn
        let hover_rect = match state {
            ResourceState::Dead => Rect::from_min_max(
                pos2(info.canvas.min.x, hatch_y),
                pos2(info.canvas.max.x, hatch_y + height), // draw hachure until the canvas max visible x
            ),
            ResourceState::Absent => Rect::from_min_max(
                pos2(info.canvas.min.x, hatch_y),
                pos2(current_time_x, hatch_y + height), // only draw hachure until current time
            ),
            _ => Rect::from_min_max(pos2(0.0, 0.0), pos2(0.0, 0.0)), // draw nothing
        };

        let hover_rect = hover_rect.intersect(chart_clip_rect);

        // We check if the mouse is hovering the hachure
        let is_hachure_hovered = if let Some(mouse_pos) = info.response.hover_pos() {
            hover_rect.contains(mouse_pos)
        } else {
            false
        };

        // Final color of the hachure
        let final_hachure_color = if is_hachure_hovered {
            hachure_color.gamma_multiply(1.5) // More visible when hovered
        } else {
            hachure_color
        };

        while x < info.canvas.max.x {
            if state == ResourceState::Absent && x >= current_time_x {
                break;
            }
            shapes.push(Shape::line_segment(
                [
                    pos2(x, hatch_y),
                    pos2(x + hachure_spacing, hatch_y + height),
                ],
                Stroke::new(2.0, final_hachure_color),
            ));
            x += hachure_spacing;
        }

        chart_painter.extend(shapes);

        // Display tooltip if hovered
        if is_hachure_hovered {
            options.current_hovered_resource_state = Some(state.clone());
        }
    }

    // Do not overlay text on job bars; keep names in the gutter and tooltips only

    if is_job_hovered {
        PaintResult::Hovered
    } else {
        PaintResult::Painted
    }
}

/**
 * Paints a job info appearing on the left side of the canvas
 */
fn paint_job_info(
    info: &Info,
    info_label: &str,
    pos: Pos2,
    collapsed: &mut bool,
    level: u8,
    gutter_width: f32,
    bar_height_hint: f32,
    label_meta: Option<LabelMeta>,
) {
    let theme_colors = get_theme_colors(&info.ctx.style());
    // Use the same painter as the canvas to avoid scroll/layer artifacts.
    let gutter_painter = info.painter.clone();

    // Disable collapsing; always expanded
    *collapsed = false;

    if let Some(meta) = label_meta {
        // Only display host rows in the gutter (hide site/cluster headers).
        let host_full = match meta.host.as_deref() {
            Some(h) if !h.trim().is_empty() => h,
            _ => return,
        };

        let indent = if level == 1 { 0.0 } else { 8.0 };
        let bar_height = bar_height_hint.max(info.text_height + 10.0);
        let top = pos.y - bar_height * 0.5;
        let left = info.canvas.min.x + 2.0 + indent;
        let total_width = (gutter_width - 4.0 - indent).max(60.0);
        let rect = Rect::from_min_max(pos2(left, top), pos2(left + total_width, top + bar_height));

        let is_hovered = info
            .response
            .hover_pos()
            .map_or(false, |mouse_pos| rect.contains(mouse_pos));

        // Clip to gutter so long labels never overlap the chart area.
        let clip_rect = Rect::from_min_max(
            pos2(info.canvas.min.x, rect.min.y),
            pos2(info.canvas.min.x + gutter_width, rect.max.y),
        );
        let gutter_painter = gutter_painter.with_clip_rect(clip_rect);

        let (label_bg, label_border, label_text_color) = if info.ctx.style().visuals.dark_mode {
            (
                Color32::from_gray(40),
                Stroke::new(1.0, Color32::from_gray(110)),
                Color32::from_gray(245),
            )
        } else {
            (
                Color32::from_rgb(240, 238, 210),
                Stroke::new(1.0, Color32::from_gray(80)),
                Color32::from_gray(20),
            )
        };

        // Paint the full gutter block (subtle colors for readability)
        gutter_painter.rect_filled(rect, 0.0, label_bg);
        gutter_painter.rect(rect, 0.0, label_bg, label_border);

        let label_text = short_host_label(host_full);

        let label_x = left + 6.0;
        let label_font = FontId::proportional((info.font_id.size - 1.0).max(11.0));
        let label_pos = pos2(label_x, rect.center().y);
        gutter_painter.text(
            label_pos,
            Align2::LEFT_CENTER,
            label_text,
            label_font,
            if is_hovered {
                theme_colors.text
            } else {
                label_text_color
            },
        );

        // Tooltip only; keep it minimal (host only)
        if is_hovered {
            info.ctx.set_cursor_icon(CursorIcon::PointingHand);
            let layer_id = LayerId::new(Order::Tooltip, Id::new("gantt-label-layer"));
            egui::containers::popup::show_tooltip(
                &info.ctx,
                layer_id,
                Id::new(format!("gantt-label-host-{}-{}", info_label, level)),
                |ui: &mut egui::Ui| {
                    ui.label(format!("host: {}", host_full));
                },
            );
        }

        return;
    }

    let label = info_label.to_string();

    let galley = info
        .ctx
        .fonts(|f| f.layout_no_wrap(label, info.font_id.clone(), theme_colors.text_dim));

    let base_x = info.canvas.min.x + 6.0; // inside gutter
    let offset_x = if level == 1 { 0.0 } else { 24.0 };
    let rect = Rect::from_min_size(pos2(base_x + offset_x, pos.y), galley.size());

    // Check if the section is hovered
    let is_hovered = if let Some(mouse_pos) = info.response.hover_pos() {
        rect.contains(mouse_pos)
    } else {
        false
    };

    // If the section is hovered, depending on the theme, we change the text color
    let text_color = if is_hovered {
        theme_colors.text
    } else {
        theme_colors.text_dim
    };

    // Clip to gutter so long labels don't spill into the chart.
    let clip_rect = Rect::from_min_max(
        pos2(info.canvas.min.x, rect.min.y - 2.0),
        pos2(info.canvas.min.x + gutter_width, rect.max.y + 2.0),
    );
    let gutter_painter = gutter_painter.with_clip_rect(clip_rect);

    // Paint the background and the text
    gutter_painter.rect_filled(rect.expand(2.0), 0.0, theme_colors.background);
    gutter_painter.galley(rect.min, galley, text_color);

    // No collapse toggle
}

/****************************************************************************************************************************/
// TIMELINE
/****************************************************************************************************************************/

/**
 * Paints the timeline text labels
 */
fn paint_timeline_text(
    info: &Info,
    canvas: Rect,
    options: &Options,
    grid_spacing_minutes: i64,
    fixed_timeline_y: f32,
    alpha_multiplier: f32,
    zoom_factor: f32,
) -> Vec<egui::Shape> {
    let mut shapes = vec![];
    let big_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.5..=1.0);
    let medium_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.1..=0.5);
    let mut grid_s = 0;

    // Paint the timeline text labels
    loop {

        // Calculate the x position of the line
        let line_x = info.point_from_s(options, grid_s);

        // If the line is out of the canvas, we stop
        if line_x > canvas.max.x {
            break;
        }
        
        // If the line is inside the canvas, we paint it
        if canvas.min.x <= line_x {
            let big_line = grid_s % (grid_spacing_minutes * 20) == 0;
            let medium_line = grid_s % (grid_spacing_minutes * 10) == 0;

            // Determine the alpha of the text
            let text_alpha = if big_line {
                big_alpha
            } else if medium_line {
                medium_alpha
            } else {
                0.0
            };

            // If the text is visible, we paint it
            if text_alpha > 0.0 {
                let text = grid_text(grid_s);
                let text_x = line_x + 4.0;
                let text_color = if info.ctx.style().visuals.dark_mode {
                    Color32::from(Rgba::from_white_alpha(
                        (text_alpha * alpha_multiplier * 2.0).min(1.0),
                    ))
                } else {
                    Color32::from(Rgba::from_black_alpha(
                        (text_alpha * alpha_multiplier * 2.0).min(1.0),
                    ))
                };

                // Paint the text
                info.painter.fonts(|f| {
                    shapes.push(egui::Shape::text(
                        f,
                        pos2(text_x, fixed_timeline_y),
                        Align2::LEFT_TOP,
                        &text,
                        info.font_id.clone(),
                        text_color,
                    ));
                });
            }
        }

        grid_s += grid_spacing_minutes;
    }

    shapes
}

fn paint_timeline_text_on_top(
    info: &Info,
    options: &Options,
    fixed_timeline_y: f32,
    gutter_width: f32,
) {
    let max_lines = info.usable_width() / 4.0;
    let mut grid_spacing_minutes = 180;
    while options.canvas_width_s / (grid_spacing_minutes as f32) > max_lines {
        grid_spacing_minutes *= 10;
    }

    let theme_colors = get_theme_colors(&info.ctx.style());

    // Determine the alpha multiplier of the lines depending on the theme
    let alpha_multiplier = if info.ctx.style().visuals.dark_mode {
        0.3
    } else {
        0.8
    };

    let num_tiny_lines = options.canvas_width_s / (grid_spacing_minutes as f32);
    let zoom_factor = remap_clamp(num_tiny_lines, (0.1 * max_lines)..=max_lines, 1.0..=0.0);

    // Paint background rect first
    let bg_rect = Rect::from_min_size(
        pos2(info.canvas.min.x + gutter_width, fixed_timeline_y),
        egui::vec2(info.usable_width(), info.text_height + 5.0),
    );
    info.painter
        .rect_filled(bg_rect, 0.0, theme_colors.background_timeline);

    // Paint timeline text on top of background
    let timeline_text = paint_timeline_text(
        info,
        info.canvas,
        options,
        grid_spacing_minutes,
        fixed_timeline_y,
        alpha_multiplier,
        zoom_factor,
    );

    // Add the shapes to the painter
    for shape in timeline_text {
        info.painter.add(shape);
    }

}

/**
 * Paints the timeline
 */
fn paint_timeline(
    info: &Info,
    canvas: Rect,
    options: &Options,
    _start_s: i64,
    _gutter_width: f32,
) -> Vec<egui::Shape> {
    let mut shapes = vec![];
    let theme_colors = get_theme_colors(&info.ctx.style());

    // Determine the alpha multiplier of the lines depending on the theme
    let alpha_multiplier = if info.ctx.style().visuals.dark_mode {
        0.3
    } else {
        0.8
    };

    let max_lines = info.usable_width() / 4.0; // Maximum number of lines that can be displayed
    let mut grid_spacing_minutes = 180; // 180 seconds = 3 minutes repesenting the grid spacing

    while options.canvas_width_s / (grid_spacing_minutes as f32) > max_lines {
        grid_spacing_minutes *= 10;
    }

    let num_tiny_lines = options.canvas_width_s / (grid_spacing_minutes as f32);
    let zoom_factor = remap_clamp(num_tiny_lines, (0.1 * max_lines)..=max_lines, 1.0..=0.0);
    let zoom_factor = zoom_factor * zoom_factor;
    let big_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.5..=1.0);
    let medium_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.1..=0.5);
    let tiny_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.0..=0.1);

    let mut grid_s = 0;

    // Paint the timeline lines
    loop {

        // Calculate the x position of the line
        let line_x = info.point_from_s(options, grid_s);

        // If the line is out of the canvas, we stop
        if line_x > canvas.max.x {
            break;
        }

        // If the line is inside the canvas, we paint it
        if canvas.min.x <= line_x {
            let big_line = grid_s % (grid_spacing_minutes * 20) == 0;
            let medium_line = grid_s % (grid_spacing_minutes * 10) == 0;

            // Determine the alpha of the line
            let line_alpha = if big_line {
                big_alpha
            } else if medium_line {
                medium_alpha
            } else {
                tiny_alpha
            };

            // Paint the line
            shapes.push(egui::Shape::line_segment(
                [pos2(line_x, canvas.min.y), pos2(line_x, canvas.max.y)],
                Stroke::new(
                    1.0,
                    theme_colors
                        .line
                        .linear_multiply(line_alpha * alpha_multiplier),
                ),
            ));
        }

        grid_s += grid_spacing_minutes;
    }

    shapes
}

/**
 * Paints the current red time line on the canvas
 */
fn paint_current_time_line(
    info: &Info,
    options: &Options,
    canvas: Rect,
    _gutter_width: f32,
) -> egui::Shape {
    let current_time = chrono::Utc::now().timestamp(); // Current time in seconds
    let line_x = info.point_from_s(options, current_time); // Current time in pixels

    // Paint the current time line
    egui::Shape::line_segment(
        [pos2(line_x, canvas.min.y), pos2(line_x, canvas.max.y)],
        Stroke::new(2.0, Color32::RED), // Keep red for both themes for better visibility
    )
}

/**
 * Converts a timestamp to a string
 */
fn grid_text(ts: i64) -> String {
    if ts == 0 {
        "N/A".to_string()
    } else {
        if let Some(dt) = DateTime::from_timestamp(ts, 0) {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        } else {
            "Invalid timestamp".to_string()
        }
    }
}

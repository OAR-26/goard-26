use super::jobs::{
    build_resource_groups, draw_level_n, draw_stripe_borders, paint_job_id_labels, paint_tooltip,
};
use super::theme::get_theme_colors;
use super::timeline::{paint_timeline_text_on_top, timeline_header_height};
use super::types::{gutter_stripes_total_w, Info, Options};
use crate::models::data_structure::application_context::ApplicationContext;
use crate::models::data_structure::cluster::Cluster;
use crate::models::data_structure::job::Job;
use crate::views::components::job_details::JobDetailsWindow;
use egui::{pos2, Rect, Stroke};

pub(super) fn ui_canvas(
    options: &mut Options,
    app: &ApplicationContext,
    info: &Info,
    fixed_timeline_y: f32,
    (min_ns, max_ns): (i64, i64),
    details_window: &mut Vec<JobDetailsWindow>,
    all_cluster: &Vec<Cluster>,
    gutter_width: f32,
) -> f32 {
    options.hovered_leaf_label = None;

    if options.canvas_width_s <= 0.0 {
        options.canvas_width_s = (max_ns - min_ns) as f32;
        options.zoom_to_relative_s_range = None;
    }

    let mut cursor_y = info.canvas.min.y;
    let mut all_painted_rows = Vec::new();
    cursor_y += timeline_header_height(options, info.usable_width(), info.text_height);

    let theme_colors = get_theme_colors(&info.ctx.style());

    // Gutter background — neutral (yellow comes from the per-level stripes).
    let gutter_bg = if info.ctx.style().visuals.dark_mode {
        egui::Color32::from_gray(20)
    } else {
        egui::Color32::WHITE
    };

    let gutter_rect = Rect::from_min_max(
        pos2(info.canvas.min.x, info.canvas.min.y),
        pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
    );
    info.painter.rect_filled(gutter_rect, 0.0, gutter_bg);

    // ── Build hierarchy and draw recursively ──────────────────────────────
    let jobs_refs: Vec<&Job> = app.data.filtered_jobs.iter().collect();
    let n_total = options.levels.len();
    let label_x_end = gutter_width - gutter_stripes_total_w(n_total);
    let stripe_x_start = info.canvas.min.x + label_x_end;

    if n_total > 0 {
        let levels = options.levels.clone();
        let groups = build_resource_groups(app, &levels, &jobs_refs, options.resource_filter.as_ref(), options.leaf_label_template.as_deref());

        let stripes_y_start = cursor_y;
        cursor_y = draw_level_n(
            info,
            options,
            &groups,
            &levels,
            0,
            n_total,
            stripe_x_start,
            label_x_end,
            gutter_width,
            cursor_y,
            details_window,
            app,
            all_cluster,
            &mut all_painted_rows,
            &[],
        );

        draw_stripe_borders(info, n_total, gutter_width, stripes_y_start, cursor_y);
    }

    // Vertical separator between gutter and chart area.
    info.painter.line_segment(
        [
            pos2(info.canvas.min.x + gutter_width, info.canvas.min.y),
            pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
        ],
        Stroke::new(1.0, theme_colors.line),
    );

    paint_job_id_labels(info, options, &all_painted_rows);
    paint_tooltip(info, options, app);
    options.previous_hovered_job = options.current_hovered_job.clone();
    options.current_hovered_job = None;
    paint_timeline_text_on_top(info, options, fixed_timeline_y, gutter_width);

    cursor_y
}

use super::labels::{
    build_label_meta_level1, build_label_meta_level2, short_host_label, site_for_cluster_name,
    LabelMeta,
};
use super::theme::get_theme_colors;
use super::types::{Info, Options};
use crate::models::data_structure::cluster::Cluster;
use crate::models::data_structure::job::Job;
use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::date_converter::format_timestamp;
use crate::models::utils::utils::{
    compare_string_with_number, get_cluster_state_from_name, get_host_state_from_name,
    get_tree_structure_for_job,
};
use crate::views::components::gantt_aggregate_by::{AggregateByLevel1Enum, AggregateByLevel2Enum};
use crate::views::components::job_details::JobDetailsWindow;
use egui::{
    pos2, Align2, Color32, CursorIcon, FontId, Id, LayerId, Order, Pos2, Rect, Shape, Stroke,
};
use std::collections::BTreeMap;

pub(super) fn paint_tooltip(info: &Info, options: &mut Options) {
    let mut tooltip_text = String::new();

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
        options.current_hovered_resource_state = None;
    }

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

pub(super) fn paint_aggregated_jobs_level_1<'a>(
    info: &Info,
    options: &mut Options,
    jobs: BTreeMap<String, Vec<&'a Job>>,
    mut cursor_y: f32,
    details_window: &mut Vec<JobDetailsWindow>,
    collapsed_jobs: &mut BTreeMap<String, bool>,
    font_size: i32,
    all_cluster: &Vec<Cluster>,
    aggregate_by: AggregateByLevel1Enum,
    gutter_width: f32,
) -> f32 {
    let theme_colors = get_theme_colors(&info.ctx.style());

    let spacing_between_level_1 = font_size as f32 * 0.25;
    let spacing_between_jobs = 0.0;
    let offset_level_1 = 6.0;

    cursor_y += spacing_between_level_1;

    let mut sorted_level_1: Vec<String> = jobs.keys().cloned().collect();
    sorted_level_1.sort_by(|a, b| compare_string_with_number(a, b));

    let aggregation_height = font_size as f32 + 5.0 + offset_level_1;

    let mut header_data: Vec<(String, Pos2, bool, Option<LabelMeta>)> = Vec::new();

    for level_1 in sorted_level_1 {
        let job_list = jobs.get(&level_1).unwrap();

        info.painter.line_segment(
            [
                pos2(info.canvas.min.x, cursor_y),
                pos2(info.canvas.max.x, cursor_y),
            ],
            Stroke::new(1.5, theme_colors.aggregated_line_level_1),
        );

        cursor_y += offset_level_1;

        let text_pos = pos2(info.canvas.min.x, cursor_y);

        let is_collapsed = collapsed_jobs.entry(level_1.clone()).or_insert(false);
        *is_collapsed = false;
        let label_meta = build_label_meta_level1(&level_1, aggregate_by, all_cluster);

        if options.squash_resources {
            header_data.push((level_1.clone(), text_pos, *is_collapsed, label_meta));
        } else {
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

        cursor_y += spacing_between_level_1;

        let state = if aggregate_by == AggregateByLevel1Enum::Owner {
            ResourceState::Alive
        } else if aggregate_by == AggregateByLevel1Enum::Host {
            get_host_state_from_name(all_cluster, &level_1)
        } else {
            get_cluster_state_from_name(all_cluster, &level_1)
        };

        if !*is_collapsed {
            let initial_job_y = cursor_y;

            for job in job_list {
                let job_start_y = if options.squash_resources {
                    initial_job_y
                } else {
                    cursor_y
                };

                paint_job(
                    info,
                    options,
                    job,
                    job_start_y,
                    details_window,
                    all_cluster,
                    state,
                    aggregation_height,
                );

                if !options.squash_resources {
                    cursor_y += info.text_height + spacing_between_jobs + options.spacing;
                }
            }

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

    if options.squash_resources {
        for (name, pos, is_collapsed, label_meta) in header_data {
            let galley = info.ctx.fonts(|f| {
                let collapsed_symbol = if is_collapsed { "⏵" } else { "⏷" };
                let label = format!("{} {}", collapsed_symbol, name);
                f.layout_no_wrap(label, info.font_id.clone(), theme_colors.text_dim)
            });
            let rect = Rect::from_min_size(pos, galley.size());
            info.painter
                .rect_filled(rect.expand(4.0), 4.0, theme_colors.background_timeline);

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
            if is_collapsed_copy != is_collapsed {
                *collapsed_jobs.get_mut(&name).unwrap() = is_collapsed_copy;
            }
        }
    }

    cursor_y
}

pub(super) fn paint_aggregated_jobs_level_2<'a>(
    info: &Info,
    options: &mut Options,
    jobs: BTreeMap<String, BTreeMap<String, Vec<&'a Job>>>,
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

    let mut current_site: Option<(String, f32, f32)> = None;

    let spacing_between_level_1 = font_size as f32 * 0.25;
    let spacing_between_level_2 = font_size as f32 * 0.35;
    let spacing_between_jobs = 0.0;
    let offset_level_1 = 6.0;

    cursor_y += spacing_between_level_1;

    let mut sorted_level_1: Vec<String> = jobs.keys().cloned().collect();
    sorted_level_1.sort_by(|a, b| compare_string_with_number(a, b));

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
            info.painter.line_segment(
                [
                    pos2(info.canvas.min.x, cursor_y),
                    pos2(info.canvas.max.x, cursor_y),
                ],
                Stroke::new(1.5, theme_colors.aggregated_line_level_1),
            );

            cursor_y += offset_level_1;

            let text_pos = pos2(info.canvas.min.x, cursor_y);

            let is_collapsed_level_1 = collapsed_jobs_level_1
                .entry(level_1.clone())
                .or_insert(false);
            *is_collapsed_level_1 = false;
            let label_meta_level_1 =
                build_label_meta_level1(&level_1, aggregate_by_level_1, all_cluster);

            if options.squash_resources {
                header_data_level_1.push((
                    level_1.clone(),
                    text_pos,
                    *is_collapsed_level_1,
                    label_meta_level_1,
                ));
            } else {
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
            cursor_y += spacing_between_level_1;
        }

        let is_collapsed_level_1 = collapsed_jobs_level_1
            .entry(level_1.clone())
            .or_insert(false);
        *is_collapsed_level_1 = false;

        if !*is_collapsed_level_1 {
            let mut sorted_level_2: Vec<_> = level_2_map.keys().collect();
            sorted_level_2.sort_by(|a, b| compare_string_with_number(a, b));

            for level_2 in sorted_level_2 {
                if let Some(job_list) = level_2_map.get(level_2) {
                    info.painter.line_segment(
                        [
                            pos2(info.canvas.min.x, cursor_y),
                            pos2(info.canvas.max.x, cursor_y),
                        ],
                        Stroke::new(0.5, theme_colors.aggregated_line_level_2),
                    );

                    cursor_y += spacing_between_level_2;

                    let row_center_y = cursor_y + spacing_between_level_2;

                    let text_pos = pos2(info.canvas.min.x + 20.0, row_center_y);

                    let is_collapsed_level_2 = collapsed_jobs_level_2
                        .entry((level_1_key.to_string(), level_2.to_string()))
                        .or_insert(false);
                    *is_collapsed_level_2 = false;
                    let label_meta_level_2 = build_label_meta_level2(
                        &level_1,
                        level_2,
                        aggregate_by_level_1,
                        aggregate_by_level_2,
                        all_cluster,
                    );

                    if options.squash_resources {
                        header_data_level_2.push((
                            level_1_key.clone(),
                            level_2.to_string(),
                            text_pos,
                            *is_collapsed_level_2,
                            label_meta_level_2,
                        ));
                    } else if hide_level_1_headers {
                        let row_height = options.rect_height.max(info.text_height + 10.0);
                        let row_rect = Rect::from_min_max(
                            pos2(info.canvas.min.x, row_center_y - row_height * 0.5),
                            pos2(
                                info.canvas.min.x + gutter_width,
                                row_center_y + row_height * 0.5,
                            ),
                        );

                        let host_short = short_host_label(&level_2.to_string());
                        grid5000_host_rows.push(GanttGutterHostRow {
                            host: host_short,
                            row_rect,
                        });

                        cluster_top = Some(cluster_top.unwrap_or(row_rect.min.y).min(row_rect.min.y));
                        cluster_bottom =
                            Some(cluster_bottom.unwrap_or(row_rect.max.y).max(row_rect.max.y));
                    } else {
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

                    cursor_y += spacing_between_level_2;

                    let state = if aggregate_by_level_2 == AggregateByLevel2Enum::Host {
                        get_host_state_from_name(all_cluster, level_2)
                    } else if aggregate_by_level_2 == AggregateByLevel2Enum::None {
                        if aggregate_by_level_1 == AggregateByLevel1Enum::Host {
                            get_host_state_from_name(all_cluster, &level_1)
                        } else if aggregate_by_level_1 == AggregateByLevel1Enum::Cluster {
                            get_cluster_state_from_name(all_cluster, &level_1)
                        } else {
                            ResourceState::Alive
                        }
                    } else {
                        ResourceState::Alive
                    };

                    if !*is_collapsed_level_2 {
                        let initial_job_y = cursor_y;

                        for job in job_list.iter() {
                            let aligned_y = cursor_y - options.rect_height * 0.5;
                            let job_start_y = if options.squash_resources {
                                initial_job_y
                            } else {
                                aligned_y
                            };

                            let adjusted_aggregation_height = spacing_between_level_2 * 2.0;

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

                            if !options.squash_resources && !job_list.is_empty() {
                                cursor_y += info.text_height + spacing_between_jobs;
                            }
                        }

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

        if hide_level_1_headers {
            if let (Some(top), Some(bottom)) = (cluster_top, cluster_bottom) {
                let site_w = (gutter_width * 0.28).clamp(40.0, 90.0);
                let cluster_w = (gutter_width * 0.36).clamp(60.0, 120.0);

                grid5000_cluster_spans.push(GanttGutterSpan {
                    label: level_1.clone(),
                    rect: Rect::from_min_max(
                        pos2(info.canvas.min.x + site_w, top),
                        pos2(info.canvas.min.x + site_w + cluster_w, bottom),
                    ),
                });

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

        for row in &grid5000_host_rows {
            let host_rect = Rect::from_min_max(
                pos2(info.canvas.min.x + site_w + cluster_w, row.row_rect.min.y),
                pos2(
                    info.canvas.min.x + site_w + cluster_w + host_w,
                    row.row_rect.max.y,
                ),
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

    if options.squash_resources {
        if !hide_level_1_headers {
            for (name, pos, is_collapsed, label_meta) in header_data_level_1 {
                let galley = info.ctx.fonts(|f| {
                    let collapsed_symbol = if is_collapsed { "⏵" } else { "⏷" };
                    let label = format!("{} {}", collapsed_symbol, name);
                    f.layout_no_wrap(label, info.font_id.clone(), theme_colors.text_dim)
                });
                let rect = Rect::from_min_size(pos, galley.size());
                info.painter
                    .rect_filled(rect.expand(4.0), 4.0, theme_colors.background_timeline);

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
                if is_collapsed_copy != is_collapsed {
                    *collapsed_jobs_level_1.get_mut(&name).unwrap() = is_collapsed_copy;
                }
            }
        }

        for (level_1_key, level_2_key, pos, is_collapsed, label_meta) in header_data_level_2 {
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
    let start_x = info.point_from_s(options, job.scheduled_start);
    let stop_time = if job.stop_time > 0 {
        job.stop_time
    } else {
        job.scheduled_start + job.walltime
    };
    let end_x = info.point_from_s(options, stop_time);
    let width = end_x - start_x;

    if width < options.cull_width {
        return PaintResult::Culled;
    }

    let spacing_between_jobs = 5.0;
    let total_line_height = info.text_height + spacing_between_jobs + options.spacing;

    let height = if options.squash_resources {
        total_line_height + aggregation_height
    } else {
        options.rect_height
    };

    let rounding = if options.squash_resources { 0.0 } else { options.rounding };

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

    let visible_rect = rect.intersect(chart_clip_rect);
    if visible_rect.is_negative() {
        return PaintResult::Culled;
    }

    let is_job_trully_hovered = info
        .response
        .hover_pos()
        .map_or(false, |mouse_pos| visible_rect.contains(mouse_pos));

    let is_job_hovered = info
        .response
        .hover_pos()
        .map_or(false, |mouse_pos| visible_rect.contains(mouse_pos))
        || options
            .current_hovered_job
            .as_ref()
            .map_or(false, |j| j.id == job.id)
        || options
            .previous_hovered_job
            .as_ref()
            .map_or(false, |j| j.id == job.id);

    if is_job_trully_hovered && options.current_hovered_job.is_none() {
        options.current_hovered_job = Some(job.clone());
    }

    if is_job_hovered && info.response.secondary_clicked() {
        let window = JobDetailsWindow::new(job.clone(), get_tree_structure_for_job(job, all_cluster));
        if !details_window.iter().any(|w| w.job.id == job.id) {
            details_window.push(window);
        }
    }

    if is_job_hovered && info.response.clicked() {
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

    let (hovered_color, normal_color) = if options.job_color.is_random() {
        job.get_gantt_color()
    } else {
        job.state.get_color()
    };

    let fill_color = if is_job_hovered { hovered_color } else { normal_color };

    chart_painter.rect_filled(visible_rect, rounding, fill_color);

    if state == ResourceState::Dead || state == ResourceState::Absent {
        let hachure_color = match state {
            ResourceState::Dead => Color32::from_rgba_premultiplied(255, 0, 0, 150),
            ResourceState::Absent => theme_colors.hatch,
            _ => Color32::TRANSPARENT,
        };

        let hachure_spacing = 10.0;
        let mut shapes = Vec::new();
        let mut x = info.canvas.min.x;
        let current_time_x = info.point_from_s(options, chrono::Utc::now().timestamp());

        let hatch_y = if options.squash_resources {
            top_y - aggregation_height
        } else {
            top_y
        };

        let hover_rect = match state {
            ResourceState::Dead => Rect::from_min_max(
                pos2(info.canvas.min.x, hatch_y),
                pos2(info.canvas.max.x, hatch_y + height),
            ),
            ResourceState::Absent => Rect::from_min_max(
                pos2(info.canvas.min.x, hatch_y),
                pos2(current_time_x, hatch_y + height),
            ),
            _ => Rect::from_min_max(pos2(0.0, 0.0), pos2(0.0, 0.0)),
        };

        let hover_rect = hover_rect.intersect(chart_clip_rect);

        let is_hachure_hovered = info
            .response
            .hover_pos()
            .map_or(false, |mouse_pos| hover_rect.contains(mouse_pos));

        let final_hachure_color = if is_hachure_hovered {
            hachure_color.gamma_multiply(1.5)
        } else {
            hachure_color
        };

        while x < info.canvas.max.x {
            if state == ResourceState::Absent && x >= current_time_x {
                break;
            }
            shapes.push(Shape::line_segment(
                [pos2(x, hatch_y), pos2(x + hachure_spacing, hatch_y + height)],
                Stroke::new(2.0, final_hachure_color),
            ));
            x += hachure_spacing;
        }

        chart_painter.extend(shapes);

        if is_hachure_hovered {
            options.current_hovered_resource_state = Some(state.clone());
        }
    }

    if is_job_hovered {
        PaintResult::Hovered
    } else {
        PaintResult::Painted
    }
}

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
    let gutter_painter = info.painter.clone();

    *collapsed = false;

    if let Some(meta) = label_meta {
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

    let base_x = info.canvas.min.x + 6.0;
    let offset_x = if level == 1 { 0.0 } else { 24.0 };
    let rect = Rect::from_min_size(pos2(base_x + offset_x, pos.y), galley.size());

    let is_hovered = info
        .response
        .hover_pos()
        .map_or(false, |mouse_pos| rect.contains(mouse_pos));

    let text_color = if is_hovered {
        theme_colors.text
    } else {
        theme_colors.text_dim
    };

    let clip_rect = Rect::from_min_max(
        pos2(info.canvas.min.x, rect.min.y - 2.0),
        pos2(info.canvas.min.x + gutter_width, rect.max.y + 2.0),
    );
    let gutter_painter = gutter_painter.with_clip_rect(clip_rect);

    gutter_painter.rect_filled(rect.expand(2.0), 0.0, theme_colors.background);
    gutter_painter.galley(rect.min, galley, text_color);
}

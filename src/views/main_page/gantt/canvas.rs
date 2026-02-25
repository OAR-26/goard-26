use super::jobs::{paint_aggregated_jobs_level_1, paint_aggregated_jobs_level_2, paint_tooltip};
use super::theme::get_theme_colors;
use super::timeline::paint_timeline_text_on_top;
use super::types::{Info, Options};
use crate::models::data_structure::application_context::ApplicationContext;
use crate::models::data_structure::cluster::Cluster;
use crate::models::data_structure::job::Job;
use crate::models::utils::utils::{
    cluster_contain_host, contains_cluster, contains_host, get_cluster_from_name,
};
use crate::views::components::gantt_aggregate_by::{AggregateByLevel1Enum, AggregateByLevel2Enum};
use crate::views::components::job_details::JobDetailsWindow;
use egui::{pos2, Rect, Stroke};
use std::collections::BTreeMap;

pub(super) fn ui_canvas(
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

    let theme_colors = get_theme_colors(&info.ctx.style());
    // Fond du gutter identique partout : en mode Grid5000, les colonnes (site/cluster/host)
    // sont dessinées dans jobs.rs et ne doivent pas teinter toute la zone (évite les zones jaunes vides).
    let gutter_bg = theme_colors.background;

    let gutter_rect = Rect::from_min_max(
        pos2(info.canvas.min.x, info.canvas.min.y),
        pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
    );
    info.painter.rect_filled(gutter_rect, 0.0, gutter_bg);
    info.painter.line_segment(
        [
            pos2(info.canvas.min.x + gutter_width, info.canvas.min.y),
            pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
        ],
        Stroke::new(1.0, theme_colors.line),
    );

    let jobs = &app.filtered_jobs;

    match options.aggregate_by.level_1 {
        AggregateByLevel1Enum::Owner => {
            let mut jobs_by_owner: BTreeMap<String, Vec<&Job>> = BTreeMap::new();
            for job in jobs.iter() {
                jobs_by_owner
                    .entry(job.owner.clone())
                    .or_insert_with(Vec::new)
                    .push(job);
            }

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

        AggregateByLevel1Enum::Host => match options.aggregate_by.level_2 {
            AggregateByLevel2Enum::Owner => {
                let mut jobs_by_host_by_owner: BTreeMap<String, BTreeMap<String, Vec<&Job>>> =
                    BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                for job in jobs.iter() {
                    for host in job.hosts.iter() {
                        if filtered_clusters.len() != 0 && !contains_host(&filtered_clusters, host) {
                            continue;
                        }
                        jobs_by_host_by_owner
                            .entry(host.clone())
                            .or_insert_with(BTreeMap::new)
                            .entry(job.owner.clone())
                            .or_insert_with(Vec::new)
                            .push(job);
                    }
                }

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

            AggregateByLevel2Enum::None => {
                let mut jobs_by_host: BTreeMap<String, Vec<&Job>> = BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                for job in jobs.iter() {
                    for host in job.hosts.iter() {
                        if filtered_clusters.len() != 0 && !contains_host(&filtered_clusters, host) {
                            continue;
                        }
                        jobs_by_host
                            .entry(host.clone())
                            .or_insert_with(Vec::new)
                            .push(job);
                    }
                }

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
        },

        AggregateByLevel1Enum::Cluster => match options.aggregate_by.level_2 {
            AggregateByLevel2Enum::Owner => {
                let mut jobs_by_cluster_by_owner: BTreeMap<String, BTreeMap<String, Vec<&Job>>> =
                    BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                for job in jobs.iter() {
                    for cluster in job.clusters.iter() {
                        if filtered_clusters.len() != 0
                            && !contains_cluster(&filtered_clusters, cluster)
                        {
                            continue;
                        }
                        jobs_by_cluster_by_owner
                            .entry(cluster.clone())
                            .or_insert_with(BTreeMap::new)
                            .entry(job.owner.clone())
                            .or_insert_with(Vec::new)
                            .push(job);
                    }
                }

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

            AggregateByLevel2Enum::None => {
                let mut jobs_by_cluster: BTreeMap<String, Vec<&Job>> = BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                for job in jobs.iter() {
                    for cluster in job.clusters.iter() {
                        if filtered_clusters.len() != 0
                            && !contains_cluster(&filtered_clusters, cluster)
                        {
                            continue;
                        }
                        jobs_by_cluster
                            .entry(cluster.clone())
                            .or_insert_with(Vec::new)
                            .push(job);
                    }
                }

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

            AggregateByLevel2Enum::Host => {
                let mut jobs_by_cluster_by_host: BTreeMap<String, BTreeMap<String, Vec<&Job>>> =
                    BTreeMap::new();
                let filtered_clusters = app.filters.clusters.clone().unwrap_or_default();

                for job in jobs.iter() {
                    for cluster_name in job.clusters.iter() {
                        if filtered_clusters.len() != 0
                            && !contains_cluster(&filtered_clusters, cluster_name)
                        {
                            continue;
                        }

                        let curr_cluster = match get_cluster_from_name(&app.all_clusters, cluster_name)
                        {
                            Some(c) => c,
                            None => continue,
                        };

                        for host in job.hosts.iter() {
                            if cluster_contain_host(&curr_cluster, host) {
                                jobs_by_cluster_by_host
                                    .entry(cluster_name.clone())
                                    .or_insert_with(BTreeMap::new)
                                    .entry(host.clone())
                                    .or_insert_with(Vec::new)
                                    .push(job);
                            }
                        }
                    }
                }

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

    // Preserve the original call sequence
    paint_tooltip(info, options);
    options.previous_hovered_job = options.current_hovered_job.clone();
    options.current_hovered_job = None;
    paint_timeline_text_on_top(info, options, fixed_timeline_y, gutter_width);

    cursor_y
}

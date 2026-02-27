use super::labels::{
    build_label_meta_level1, build_label_meta_level2, short_host_label, site_for_cluster_name,
    LabelMeta,
};
use super::theme::get_theme_colors;
use super::types::{Info, Options};
use crate::models::data_structure::cluster::Cluster;
use crate::models::data_structure::job::Job;
use crate::models::data_structure::resource::ResourceState;
use crate::models::data_structure::application_context::ApplicationContext;
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

fn json_value_to_inline(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            let mut parts: Vec<String> = Vec::new();
            for x in arr {
                if let Some(s) = json_value_to_inline(x) {
                    let t = s.trim();
                    if !t.is_empty() {
                        parts.push(t.to_string());
                    }
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            }
        }
        serde_json::Value::Object(_) => Some(v.to_string()),
    }
}

fn format_cpuset_grid5000(values: &mut Vec<i32>) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    values.dedup();

    if values.len() == 1 {
        return Some(values[0].to_string());
    }

    // If contiguous (step 1), compress for readability.
    let mut contiguous = true;
    for w in values.windows(2) {
        if w[1] != w[0] + 1 {
            contiguous = false;
            break;
        }
    }
    if contiguous {
        return Some(format!("{}-{}", values[0], values[values.len() - 1]));
    }

    // Non-contiguous: show explicit list like Grid5000.
    Some(values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
}

fn extract_ints_from_str(s: &str) -> Vec<i32> {
    // Extract all positive integer tokens from an arbitrary string.
    // Examples:
    // - "0, 1, 2" -> [0,1,2]
    // - "0-31" -> [0,31] (we can't infer the full range from this alone)
    let mut out: Vec<i32> = Vec::new();
    let mut cur: i64 = 0;
    let mut in_num = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            in_num = true;
            cur = cur * 10 + (ch as i64 - '0' as i64);
        } else if in_num {
            if cur <= i32::MAX as i64 {
                out.push(cur as i32);
            }
            cur = 0;
            in_num = false;
        }
    }
    if in_num && cur <= i32::MAX as i64 {
        out.push(cur as i32);
    }
    out
}

fn cpuset_like_grid5000(s: &crate::models::data_structure::strata::Strata) -> Option<String> {
    // Goal: display like Grid5000: a compact list of CPU indexes.
    // - If cpuset is an explicit list: compact it into ranges.
    // - If cpuset is a scalar/string in OAR resources: derive indexes from core_count (preferred) or thread_count.
    // - Format is bracketed ranges: [0-31] or [0-3, 5, 7-9]

    if let Some(v) = s.cpuset.as_ref() {
        match v {
            serde_json::Value::Array(arr) => {
                let mut ints: Vec<i32> = Vec::new();
                for x in arr {
                    match x {
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                if (0..=i32::MAX as i64).contains(&i) {
                                    ints.push(i as i32);
                                }
                            }
                        }
                        serde_json::Value::String(s) => {
                            for i in extract_ints_from_str(s) {
                                ints.push(i);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(s) = format_cpuset_grid5000(&mut ints) {
                    return Some(s);
                }
            }
            serde_json::Value::String(raw) => {
                // If we get a comma-separated string list, compact it.
                let mut ints = extract_ints_from_str(raw);
                if ints.len() > 1 {
                    if let Some(s) = format_cpuset_grid5000(&mut ints) {
                        return Some(s);
                    }
                }
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    if (0..=i32::MAX as i64).contains(&i) {
                        return Some(format!("[{}]", i));
                    }
                }
            }
            _ => {}
        }
    }

    let count = s.core_count.or(s.thread_count).unwrap_or(0);
    if count <= 0 {
        return None;
    }
    // Fallback when we don't have an explicit list: assume 0..count-1.
    Some(format!("0-{}", count - 1))
}

pub(super) fn paint_tooltip(info: &Info, options: &mut Options, app: &ApplicationContext) {
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

        let kind_label = if (options.aggregate_by.level_2 == AggregateByLevel2Enum::None
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
        };

        if let Some(label) = options.current_hovered_resource_label.as_deref() {
            let trimmed = label.trim();
            if !trimmed.is_empty() {
                tooltip_text.push_str(&format!("{}: {}\n", kind_label.to_lowercase(), trimmed));

                // Grid5000-like: show the resource/host metadata when available.
                let mut keys_to_try: Vec<String> = Vec::new();
                keys_to_try.push(trimmed.to_string());
                keys_to_try.push(trimmed.split('.').next().unwrap_or(trimmed).to_string());
                for key in keys_to_try {
                    if let Some(s) = app.strata_by_host.get(&key) {
                        if let Some(cluster) = s.cluster.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                            tooltip_text.push_str(&format!("cluster: {}\n", cluster));
                        }
                        if let Some(net) = s
                            .network_address
                            .as_deref()
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                        {
                            tooltip_text.push_str(&format!("network_address: {}\n", net));
                        }
                        if let Some(comment) = s.comment.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                            tooltip_text.push_str(&format!("comment: {}\n", comment));
                        }
                        if let Some(cpuset) = cpuset_like_grid5000(s) {
                            let cpuset = cpuset.trim();
                            if !cpuset.is_empty() {
                                tooltip_text.push_str(&format!("cpuset: {}\n", cpuset));
                            }
                        }
                        if let Some(model) = s.nodemodel.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                            tooltip_text.push_str(&format!("nodemodel: {}\n", model));
                        }
                        if let Some(cpu) = s.cputype.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                            tooltip_text.push_str(&format!("cputype: {}\n", cpu));
                        }
                        if let Some(rid) = s.resource_id {
                            tooltip_text.push_str(&format!("resource_id: {}\n", rid));
                        }
                        break;
                    }
                }
            }
        }

        tooltip_text.push_str(&format!(
            "{} State: {:?}",
            kind_label,
            resource_state
        ));
        options.current_hovered_resource_state = None;
        options.current_hovered_resource_label = None;
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
    app: &ApplicationContext,
) -> f32 {
    let theme_colors = get_theme_colors(&info.ctx.style());

    let compact = options.compact_rows;
    let row_height = options.rect_height.max(info.text_height);

    let spacing_between_level_1 = if compact { 0.0 } else { font_size as f32 * 0.25 };
    let spacing_between_jobs = 0.0;
    let offset_level_1 = if compact { 0.0 } else { 6.0 };

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

        // paint_job_info expects a center Y
        let text_pos = pos2(info.canvas.min.x + 6.0, cursor_y + info.text_height * 0.5);

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
                compact,
                label_meta,
                app,
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

        let resource_label_for_state_tooltip = match aggregate_by {
            AggregateByLevel1Enum::Host | AggregateByLevel1Enum::Cluster => Some(level_1.as_str()),
            AggregateByLevel1Enum::Owner => None,
        };

        if !*is_collapsed {
            // Une seule ligne par groupe (level_1) : tous les jobs sont peints sur la même ligne
            let initial_job_y = cursor_y;
            let job_row_y = if options.squash_resources {
                initial_job_y
            } else {
                cursor_y
            };

            for job in job_list {
                paint_job(
                    info,
                    options,
                    job,
                    job_row_y,
                    details_window,
                    all_cluster,
                    state,
                    aggregation_height,
                    resource_label_for_state_tooltip,
                );
            }

            if !job_list.is_empty() {
                cursor_y += row_height + spacing_between_jobs + options.spacing;
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
                compact,
                label_meta,
                app,
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
    app: &ApplicationContext,
) -> f32 {
    let theme_colors = get_theme_colors(&info.ctx.style());

    let compact = options.compact_rows;
    // In Host -> Owner with compact mode, owner badges need a bit more vertical room to avoid overlap.
    let extra_row_pad = if compact
        && aggregate_by_level_1 == AggregateByLevel1Enum::Host
        && aggregate_by_level_2 == AggregateByLevel2Enum::Owner
    {
        8.0
    } else {
        0.0
    };
    let row_height = options.rect_height.max(info.text_height + extra_row_pad);

    let hide_level_1_headers = aggregate_by_level_1 == AggregateByLevel1Enum::Cluster
        && aggregate_by_level_2 == AggregateByLevel2Enum::Host;

    #[derive(Clone)]
    struct GanttGutterHostRow {
        host_short: String,
        host_full: String,
        cluster: String,
        site: String,
        row_rect: Rect,
    }

    #[derive(Clone)]
    struct GanttGutterSpan {
        label: String,
        top: f32,
        bottom: f32,
    }

    let mut grid5000_host_rows: Vec<GanttGutterHostRow> = Vec::new();
    let mut grid5000_cluster_spans: Vec<GanttGutterSpan> = Vec::new();
    let mut grid5000_site_spans: Vec<GanttGutterSpan> = Vec::new();

    let mut current_site: Option<(String, f32, f32)> = None;

    let spacing_between_level_1 = if compact { 0.0 } else { font_size as f32 * 0.25 };
    let spacing_between_level_2 = if compact { 0.0 } else { font_size as f32 * 0.35 };
    let spacing_between_jobs = 0.0;
    let offset_level_1 = if compact { 0.0 } else { 6.0 };

    cursor_y += spacing_between_level_1;

    let mut sorted_level_1: Vec<String> = jobs.keys().cloned().collect();
    sorted_level_1.sort_by(|a, b| compare_string_with_number(a, b));

    let mut header_data_level_1: Vec<(String, Pos2, bool, Option<LabelMeta>)> = Vec::new();
    let mut header_data_level_2: Vec<(String, String, Pos2, bool, Option<LabelMeta>)> = Vec::new();

    for level_1 in sorted_level_1 {
        let level_1_section_top = cursor_y;
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

            // In compact mode, we MUST reserve vertical space for the level_1 header row.
            // Otherwise the header label is drawn at the same Y as the first level_2 row,
            // producing visible overlaps.
            let header_height = if compact {
                options.rect_height.max(info.text_height + 2.0)
            } else {
                info.text_height
            };
            let header_center_y = cursor_y + header_height * 0.5;

            // paint_job_info expects a center Y
            let text_pos = pos2(info.canvas.min.x + 6.0, header_center_y);

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
                    compact,
                    label_meta_level_1,
                    app,
                );
            }

            if compact {
                cursor_y += header_height;
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

                    let row_center_y = if compact {
                        cursor_y + row_height * 0.5
                    } else {
                        cursor_y + spacing_between_level_2
                    };

                    let indent_x = if aggregate_by_level_1 == AggregateByLevel1Enum::Host {
                        32.0
                    } else {
                        20.0
                    };
                    let text_pos = pos2(info.canvas.min.x + indent_x, row_center_y);

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
                        let extra_pad = if compact { 2.0 } else { 10.0 };
                        let row_height = options.rect_height.max(info.text_height + extra_pad);
                        let row_rect = Rect::from_min_max(
                            pos2(info.canvas.min.x, row_center_y - row_height * 0.5),
                            pos2(
                                info.canvas.min.x + gutter_width,
                                row_center_y + row_height * 0.5,
                            ),
                        );

                        let host_short = short_host_label(&level_2.to_string());
                        grid5000_host_rows.push(GanttGutterHostRow {
                            host_short,
                            host_full: level_2.to_string(),
                            cluster: level_1.clone(),
                            site: cluster_site.clone(),
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
                            compact,
                            label_meta_level_2,
                            app,
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
                        // Une seule ligne par host/owner/etc (level_2) : tous les jobs sont peints sur la même ligne
                        let initial_job_y = cursor_y;
                        let aligned_y = if compact {
                            cursor_y
                        } else {
                            cursor_y - options.rect_height * 0.5
                        };
                        let job_row_y = if options.squash_resources {
                            initial_job_y
                        } else {
                            aligned_y
                        };

                        let adjusted_aggregation_height = spacing_between_level_2 * 2.0;

                        for job in job_list.iter() {
                            paint_job(
                                info,
                                options,
                                job,
                                job_row_y,
                                details_window,
                                all_cluster,
                                state,
                                adjusted_aggregation_height,
                                if aggregate_by_level_2 == AggregateByLevel2Enum::Host {
                                    Some(level_2.as_str())
                                } else if aggregate_by_level_2 == AggregateByLevel2Enum::None {
                                    if aggregate_by_level_1 == AggregateByLevel1Enum::Host
                                        || aggregate_by_level_1 == AggregateByLevel1Enum::Cluster
                                    {
                                        Some(level_1.as_str())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                },
                            );
                        }

                        if !job_list.is_empty() {
                            let row_spacing = if compact
                                && aggregate_by_level_1 == AggregateByLevel1Enum::Host
                                && aggregate_by_level_2 == AggregateByLevel2Enum::Owner
                            {
                                2.0
                            } else {
                                options.spacing
                            };
                            cursor_y += row_height + spacing_between_jobs + row_spacing;
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

        // Hover feedback for Host aggregation: show a clear left marker for the whole host section
        // even when hovering the job bars area (not only the label widget).
        if aggregate_by_level_1 == AggregateByLevel1Enum::Host
            && aggregate_by_level_2 == AggregateByLevel2Enum::Owner
        {
            let section_bottom = cursor_y;
            if let Some(mouse) = info.response.hover_pos() {
                if mouse.y >= level_1_section_top
                    && mouse.y <= section_bottom
                    && mouse.x >= info.canvas.min.x
                    && mouse.x <= info.canvas.max.x
                {
                    let visuals = info.ctx.style().visuals.clone();
                    let fill = visuals.selection.bg_fill;
                    let stroke = visuals.selection.stroke.color;
                    let alpha_fill = Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), 70);

                    let gutter_clip = Rect::from_min_max(
                        info.canvas.min,
                        pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
                    );
                    let gutter_painter = info.painter.with_clip_rect(gutter_clip);

                    let marker_rect = Rect::from_min_max(
                        pos2(info.canvas.min.x, level_1_section_top),
                        pos2(info.canvas.min.x + 4.0, section_bottom),
                    );
                    gutter_painter.rect_filled(marker_rect, 0.0, alpha_fill);

                    // Guide line at current pointer Y
                    info.painter.line_segment(
                        [
                            pos2(info.canvas.min.x + gutter_width, mouse.y),
                            pos2(info.canvas.max.x, mouse.y),
                        ],
                        Stroke::new(1.0, stroke),
                    );
                }
            }
        }

        if hide_level_1_headers {
            if let (Some(top), Some(bottom)) = (cluster_top, cluster_bottom) {
                grid5000_cluster_spans.push(GanttGutterSpan {
                    label: level_1.clone(),
                    top,
                    bottom,
                });

                if let Some((label, s_top, s_bottom)) = current_site.as_mut() {
                    if *label == cluster_site {
                        *s_bottom = (*s_bottom).max(bottom);
                    } else {
                        grid5000_site_spans.push(GanttGutterSpan {
                            label: label.clone(),
                            top: *s_top,
                            bottom: *s_bottom,
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
            grid5000_site_spans.push(GanttGutterSpan {
                label,
                top,
                bottom,
            });
        }
    }

    if hide_level_1_headers {
        let gutter_clip = Rect::from_min_max(
            info.canvas.min,
            pos2(info.canvas.min.x + gutter_width, info.canvas.max.y),
        );
        let gutter_painter = info.painter.with_clip_rect(gutter_clip);

        // Compute gutter widths for site, cluster, host columns based on max label width
        // Calcul dynamique de la largeur du gutter selon la taille max des labels
        let mut max_site = "site".to_string();
        let mut max_cluster = "cluster".to_string();
        let mut max_host = "host".to_string();
        for row in &grid5000_host_rows {
            if row.site.len() > max_site.len() { max_site = row.site.clone(); }
            if row.cluster.len() > max_cluster.len() { max_cluster = row.cluster.clone(); }
            if row.host_full.len() > max_host.len() { max_host = row.host_full.clone(); }
        }
        let font_site = FontId::proportional((info.font_id.size + 1.0).max(12.0));
        let font_cluster = FontId::proportional((info.font_id.size + 1.0).max(12.0));
        let font_host = FontId::proportional((info.font_id.size).max(11.0));
        let pad = 8.0;
        let site_w =
            info.ctx
                .fonts(|f| f.layout_no_wrap(max_site.clone(), font_site.clone(), Color32::BLACK).size().x)
                + pad;
        let cluster_w =
            info.ctx
                .fonts(|f| {
                    f.layout_no_wrap(max_cluster.clone(), font_cluster.clone(), Color32::BLACK)
                        .size()
                        .x
                })
                + pad;
        let mut host_w =
            info.ctx
                .fonts(|f| f.layout_no_wrap(max_host.clone(), font_host.clone(), Color32::BLACK).size().x)
                + pad;

        // If compute_gutter_width clamped to a minimum, fill remaining space with the host column
        let sum_w = site_w + cluster_w + host_w;
        if sum_w < gutter_width {
            host_w += gutter_width - sum_w;
        }

        let visuals = info.ctx.style().visuals.clone();
        let selection = visuals.selection.stroke.color;
        let (c_site, c_cluster, c_host, c_border, c_text) = if visuals.dark_mode {
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

        // font_site/font_cluster/font_host already defined above

        for span in &grid5000_site_spans {
            // N'affiche que si le label n'est pas vide et qu'il y a des jobs
            if !span.label.trim().is_empty() && (span.bottom - span.top) > 0.0 {
                let rect = Rect::from_min_max(
                    pos2(info.canvas.min.x, span.top),
                    pos2(info.canvas.min.x + site_w, span.bottom),
                );
                gutter_painter.rect_filled(rect, 0.0, c_site);
                gutter_painter.rect(rect, 0.0, c_site, Stroke::new(1.0, c_border));
                let clip = gutter_painter.with_clip_rect(rect);
                clip.text(
                    pos2(rect.min.x + 6.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &span.label,
                    font_site.clone(),
                    c_text,
                );
            }
        }

        for span in &grid5000_cluster_spans {
            if span.label.trim().is_empty() || (span.bottom - span.top) <= 0.0 {
                continue;
            }
            let rect = Rect::from_min_max(
                pos2(info.canvas.min.x + site_w, span.top),
                pos2(info.canvas.min.x + site_w + cluster_w, span.bottom),
            );
            gutter_painter.rect_filled(rect, 0.0, c_cluster);
            gutter_painter.rect(rect, 0.0, c_cluster, Stroke::new(1.0, c_border));
            let clip = gutter_painter.with_clip_rect(rect);
            clip.text(
                pos2(rect.min.x + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                &span.label,
                font_cluster.clone(),
                c_text,
            );
        }

        for row in &grid5000_host_rows {
            // N'affiche que si host a un label non vide et une hauteur > 0
            if !row.host_short.trim().is_empty() && row.row_rect.height() > 0.0 {
                let host_rect = Rect::from_min_max(
                    pos2(info.canvas.min.x + site_w + cluster_w, row.row_rect.min.y),
                    pos2(
                        info.canvas.min.x + site_w + cluster_w + host_w,
                        row.row_rect.max.y,
                    ),
                );
                // Définition locale de is_hovered
                let is_hovered = info.response.hover_pos().map_or(false, |mouse_pos| host_rect.contains(mouse_pos));

                gutter_painter.rect_filled(host_rect, 0.0, c_host);

                // Hover indicator: stronger border + left marker + horizontal guide line
                let border_stroke = if is_hovered {
                    Stroke::new(2.0, selection)
                } else {
                    Stroke::new(1.0, c_border)
                };
                gutter_painter.rect(host_rect, 0.0, c_host, border_stroke);

                if is_hovered {
                    let marker_rect = Rect::from_min_max(
                        pos2(info.canvas.min.x, host_rect.min.y),
                        pos2(info.canvas.min.x + 4.0, host_rect.max.y),
                    );
                    gutter_painter.rect_filled(marker_rect, 0.0, visuals.selection.bg_fill);

                    info.painter.line_segment(
                        [
                            pos2(info.canvas.min.x + gutter_width, host_rect.center().y),
                            pos2(info.canvas.max.x, host_rect.center().y),
                        ],
                        Stroke::new(1.0, selection),
                    );
                }

                let clip = gutter_painter.with_clip_rect(host_rect);
                clip.text(
                    pos2(host_rect.min.x + 6.0, host_rect.center().y),
                    Align2::LEFT_CENTER,
                    &row.host_short,
                    font_host.clone(),
                    c_text,
                );
                if is_hovered {
                    info.ctx.set_cursor_icon(CursorIcon::PointingHand);
                    let layer_id = egui::LayerId::new(
                        egui::Order::Tooltip,
                        egui::Id::new("gantt-grid5000-host-tooltip-layer"),
                    );
                    egui::containers::popup::show_tooltip_at_pointer(
                        &info.ctx,
                        layer_id,
                        egui::Id::new(format!("gantt-grid5000-host-tooltip:{}", row.host_full)),
                        |ui| {
                            ui.label(format!("host: {}", row.host_full));
                            if !row.cluster.trim().is_empty() {
                                ui.label(format!("cluster: {}", row.cluster));
                            }
                            if !row.site.trim().is_empty() {
                                ui.label(format!("site: {}", row.site));
                            }

                            let key_full = row.host_full.trim();
                            let key_short = short_host_label(key_full);
                            if let Some(s) = app
                                .strata_by_host
                                .get(key_full)
                                .or_else(|| app.strata_by_host.get(&key_short))
                            {
                                if let Some(b) = s
                                    .besteffort
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("besteffort: {}", b));
                                }
                                if let Some(comment) = s
                                    .comment
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("comment: {}", comment));
                                }
                                if let Some(cpuset) = cpuset_like_grid5000(s) {
                                    let cpuset = cpuset.trim();
                                    if !cpuset.is_empty() {
                                        ui.label(format!("cpuset: {}", cpuset));
                                    }
                                }
                                if let Some(dep) = s
                                    .deploy
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("deploy: {}", dep));
                                }
                                if let Some(dr) = s
                                    .drain
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("drain: {}", dr));
                                }
                                if let Some(g) = s.gpudevice.as_ref().and_then(json_value_to_inline)
                                {
                                    let g = g.trim();
                                    if !g.is_empty() {
                                        ui.label(format!("gpudevice: {}", g));
                                    }
                                }
                                if let Some(net) = s
                                    .network_address
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("network_address: {}", net));
                                }
                                if let Some(t) = s
                                    .r#type
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("type: {}", t));
                                }
                                if let Some(cpu) = s
                                    .cputype
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("cputype: {}", cpu));
                                }
                                if let Some(model) = s
                                    .nodemodel
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|v| !v.is_empty())
                                {
                                    ui.label(format!("nodemodel: {}", model));
                                }
                            }
                        },
                    );
                }
            }

            // Suppression du bloc redondant : is_hovered n'est accessible que dans le bloc précédent
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
                    compact,
                    label_meta,
                    app,
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
                compact,
                label_meta,
                app,
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
    resource_label_for_state_tooltip: Option<&str>,
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

    let base_line_height = options.rect_height.max(info.text_height);
    let spacing_between_jobs = if options.compact_rows { 0.0 } else { 5.0 };
    let total_line_height = base_line_height + spacing_between_jobs + options.spacing;

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
            if let Some(label) = resource_label_for_state_tooltip {
                if !label.trim().is_empty() {
                    options.current_hovered_resource_label = Some(label.to_string());
                }
            }
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
    compact: bool,
    label_meta: Option<LabelMeta>,
    app: &ApplicationContext,
) {
    let theme_colors = get_theme_colors(&info.ctx.style());
    let gutter_painter = info.painter.clone();

    let visuals = info.ctx.style().visuals.clone();
    let selection_stroke = visuals.selection.stroke.color;
    let selection_fill = visuals.selection.bg_fill;

    *collapsed = false;

    if let Some(meta) = label_meta {
        let host_full = match meta.host.as_deref() {
            Some(h) if !h.trim().is_empty() => h,
            _ => return,
        };

        let indent = if level == 1 { 0.0 } else { 8.0 };
        let extra_pad = if compact { 2.0 } else { 10.0 };
        let bar_height = bar_height_hint.max(info.text_height + extra_pad);
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

        let (label_bg, label_border, label_text_color) = if visuals.dark_mode {
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
        let border = if is_hovered {
            Stroke::new(2.0, selection_stroke)
        } else {
            label_border
        };
        gutter_painter.rect(rect, 0.0, label_bg, border);

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

        // Clear hover indication for host labels (works also in Aggregate by Host)
        if is_hovered {
            let marker_rect = Rect::from_min_max(
                pos2(info.canvas.min.x, rect.min.y),
                pos2(info.canvas.min.x + 4.0, rect.max.y),
            );
            gutter_painter.rect_filled(marker_rect, 0.0, selection_fill);

            info.painter.line_segment(
                [
                    pos2(info.canvas.min.x + gutter_width, rect.center().y),
                    pos2(info.canvas.max.x, rect.center().y),
                ],
                Stroke::new(1.0, selection_stroke),
            );
        }

        if is_hovered {
            info.ctx.set_cursor_icon(CursorIcon::PointingHand);
            let layer_id = LayerId::new(Order::Tooltip, Id::new("gantt-label-layer"));
            egui::containers::popup::show_tooltip(
                &info.ctx,
                layer_id,
                Id::new(format!("gantt-label-host-{}-{}", info_label, level)),
                |ui: &mut egui::Ui| {
                    ui.label(format!("host: {}", host_full));

                    let key_full = host_full.trim();
                    let key_short = short_host_label(key_full);
                    let strata = app
                        .strata_by_host
                        .get(key_full)
                        .or_else(|| app.strata_by_host.get(&key_short));

                    // Keep the 3 first lines like Grid5000: host / cluster / site
                    let derived_cluster = key_short.split('-').next().unwrap_or("").trim();
                    let cluster_line = strata
                        .and_then(|s| s.cluster.as_deref())
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .or_else(|| if !derived_cluster.is_empty() { Some(derived_cluster) } else { None });
                    if let Some(cluster) = cluster_line {
                        ui.label(format!("cluster: {}", cluster));
                    }

                    let site_from_host = host_full.split('.').nth(1).unwrap_or("").trim();
                    let site_from_net = strata
                        .and_then(|s| s.network_address.as_deref())
                        .and_then(|net| net.split('.').nth(1))
                        .unwrap_or("")
                        .trim();
                    let site = if !site_from_host.is_empty() {
                        site_from_host
                    } else {
                        site_from_net
                    };
                    if !site.is_empty() {
                        ui.label(format!("site: {}", site));
                    }

                    if let Some(s) = strata {
                        if let Some(b) =
                            s.besteffort.as_deref().map(str::trim).filter(|v| !v.is_empty())
                        {
                            ui.label(format!("besteffort: {}", b));
                        }
                        if let Some(net) = s
                            .network_address
                            .as_deref()
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                        {
                            ui.label(format!("network_address: {}", net));
                        }
                        if let Some(comment) =
                            s.comment.as_deref().map(str::trim).filter(|v| !v.is_empty())
                        {
                            ui.label(format!("comment: {}", comment));
                        }
                        if let Some(cpuset) = cpuset_like_grid5000(s) {
                            let cpuset = cpuset.trim();
                            if !cpuset.is_empty() {
                                ui.label(format!("cpuset: {}", cpuset));
                            }
                        }
                        if let Some(dep) =
                            s.deploy.as_deref().map(str::trim).filter(|v| !v.is_empty())
                        {
                            ui.label(format!("deploy: {}", dep));
                        }
                        if let Some(dr) = s.drain.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                            ui.label(format!("drain: {}", dr));
                        }
                        if let Some(g) = s.gpudevice.as_ref().and_then(json_value_to_inline) {
                            let g = g.trim();
                            if !g.is_empty() {
                                ui.label(format!("gpudevice: {}", g));
                            }
                        }
                        if let Some(t) =
                            s.r#type.as_deref().map(str::trim).filter(|v| !v.is_empty())
                        {
                            ui.label(format!("type: {}", t));
                        }
                        if let Some(cpu) =
                            s.cputype.as_deref().map(str::trim).filter(|v| !v.is_empty())
                        {
                            ui.label(format!("cputype: {}", cpu));
                        }
                        if let Some(model) =
                            s.nodemodel.as_deref().map(str::trim).filter(|v| !v.is_empty())
                        {
                            ui.label(format!("nodemodel: {}", model));
                        }
                    }
                },
            );
        }

        return;
    }

    let label = info_label.to_string();

    let galley = info
        .ctx
        .fonts(|f| f.layout_no_wrap(label, info.font_id.clone(), theme_colors.text_dim));

    // Use caller-provided X for proper indentation; keep a small minimum padding.
    let x = pos.x.max(info.canvas.min.x + 6.0);

    // Treat pos.y as CENTER to keep consistent alignment with host labels
    let top_left = pos2(x, pos.y - galley.size().y * 0.5);
    let rect = Rect::from_min_size(top_left, galley.size());

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

    // Badge background improves readability (notably in Host -> Owner aggregation)
    let (bg, rounding) = if level == 2 {
        (theme_colors.background_timeline, 2.0)
    } else {
        (theme_colors.background, 0.0)
    };
    let badge = rect.expand(1.0);
    gutter_painter.rect_filled(badge, rounding, bg);
    if level == 2 {
        gutter_painter.rect(badge, rounding, bg, Stroke::new(1.0, theme_colors.line));
    }
    gutter_painter.galley(rect.min, galley, text_color);
}

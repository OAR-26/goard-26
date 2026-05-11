use super::theme::get_theme_colors;
use super::types::{gutter_stripes_total_w, Info, Options, STRIPE_W};
use crate::models::data_structure::application_context::ApplicationContext;
use crate::models::data_structure::cluster::Cluster;
use crate::models::data_structure::job::Job;
use crate::models::data_structure::resource::{DeadInterval, ResourceState};
use crate::models::data_structure::strata::Strata;
use crate::models::utils::date_converter::format_timestamp;
use crate::models::utils::utils::{compare_string_with_number, get_tree_structure_for_job};
use crate::views::components::job_details::JobDetailsWindow;
use egui::{
    pos2, Align2, Color32, CursorIcon, FontId, Id, LayerId, Order, Rect, Shape, Stroke,
};
use std::collections::{BTreeMap, HashMap, HashSet};

// ---------------------------------------------------------------------------
// Tooltip helpers (cpuset / strata field display)
// ---------------------------------------------------------------------------

fn json_value_to_inline(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(json_value_to_inline)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if parts.is_empty() { None } else { Some(parts.join(", ")) }
        }
        serde_json::Value::Object(_) => Some(v.to_string()),
    }
}

fn format_cpuset_grid5000(values: &mut Vec<i32>) -> Option<String> {
    use range_set_blaze::RangeSetBlaze;
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    values.dedup();
    let mut range_set = RangeSetBlaze::<i32>::new();
    for &val in values.iter() {
        range_set.insert(val);
    }
    let parts: Vec<String> = range_set
        .ranges()
        .map(|r| {
            if r.start() == r.end() {
                r.start().to_string()
            } else {
                format!("{}-{}", r.start(), r.end())
            }
        })
        .collect();
    if parts.is_empty() { None } else { Some(parts.join(", ")) }
}

fn extract_ints_from_str(s: &str) -> Vec<i32> {
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

fn cpuset_display(s: &Strata) -> Option<String> {
    if let Some(v) = s.cpuset.as_ref() {
        match v {
            serde_json::Value::Array(arr) => {
                let mut flat: Vec<i32> = Vec::new();
                for x in arr {
                    match x {
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                if (0..=i32::MAX as i64).contains(&i) {
                                    flat.push(i as i32);
                                }
                            }
                        }
                        serde_json::Value::String(st) => {
                            flat.extend(extract_ints_from_str(st));
                        }
                        _ => {}
                    }
                }
                return format_cpuset_grid5000(&mut flat);
            }
            serde_json::Value::String(raw) => {
                let mut ints = extract_ints_from_str(raw);
                if !ints.is_empty() {
                    return format_cpuset_grid5000(&mut ints);
                }
                if !raw.trim().is_empty() {
                    return Some(raw.clone());
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
    if count <= 0 { None } else { Some(format!("0-{}", count - 1)) }
}

/// Collect cpuset values from ALL strata entries matching `key` at `leaf_field`,
/// then format the combined range. Works correctly for per-core data (hosts).
fn cpuset_display_aggregate(
    app: &ApplicationContext,
    leaf_field: &str,
    key: &str,
) -> Option<String> {
    let mut all_ints: Vec<i32> = Vec::new();
    for s in app.strata_by_resource_id.values() {
        if strata_field_value(s, leaf_field)
            .map(|v| v.trim().to_string())
            .as_deref()
            != Some(key.trim())
        {
            continue;
        }
        if let Some(v) = &s.cpuset {
            match v {
                serde_json::Value::Array(arr) => {
                    for x in arr {
                        match x {
                            serde_json::Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    if (0..=i32::MAX as i64).contains(&i) {
                                        all_ints.push(i as i32);
                                    }
                                }
                            }
                            serde_json::Value::String(st) => {
                                all_ints.extend(extract_ints_from_str(st));
                            }
                            _ => {}
                        }
                    }
                }
                serde_json::Value::String(raw) => {
                    all_ints.extend(extract_ints_from_str(raw));
                }
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        if (0..=i32::MAX as i64).contains(&i) {
                            all_ints.push(i as i32);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    format_cpuset_grid5000(&mut all_ints)
}

// ---------------------------------------------------------------------------
// Tooltip
// ---------------------------------------------------------------------------

pub(super) fn paint_tooltip(info: &Info, options: &mut Options, app: &ApplicationContext) {
    let mut tooltip_text = String::new();

    if let Some(job) = &options.current_hovered_job {
        let stop_str = if job.stop_time == 0 { "Running".to_string() } else { format_timestamp(job.stop_time) };
        let types_str = if job.job_types.is_empty() { "-".to_string() } else { job.job_types.join(", ") };
        let name_str = job.name.as_deref().unwrap_or("-");
        let resources_str = format!("{} resources", job.assigned_resources.len());
        let machines_str = job.hosts.len().to_string();
        let h = job.walltime / 3600;
        let m = (job.walltime % 3600) / 60;
        let s = job.walltime % 60;
        let walltime_str = format!("{}h {:02}m {:02}s", h, m, s);
        tooltip_text.push_str(&format!(
            "Job: {}\nUser: {}\nKind: {}\nQueue: {}\nTypes: {}\nName: {}\nProject: {}\nWalltime: {}\nResources: {}\nMachines: {}\nSubmission: {}\nStart: {}\nStop: {}\nState: {}",
            job.id,
            job.owner,
            job.job_type,
            job.queue,
            types_str,
            name_str,
            job.project,
            walltime_str,
            resources_str,
            machines_str,
            format_timestamp(job.submission_time),
            format_timestamp(job.scheduled_start),
            stop_str,
            job.state.get_label(),
        ));
    }

    if let Some(resource_state) = &options.current_hovered_resource_state {
        if !tooltip_text.is_empty() {
            tooltip_text.push('\n');
        }

        let kind_label = options.leaf_info_preset.as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("Resource");

        if let Some(label) = options.current_hovered_resource_label.as_deref() {
            let trimmed = label.trim();
            if !trimmed.is_empty() {
                let mut preset_fields: Vec<&str> = options.leaf_info_preset.as_ref()
                    .map(|p| p.fields.iter().map(|s| s.as_str()).collect())
                    .unwrap_or_default();
                preset_fields.sort_unstable();
                if !preset_fields.is_empty() {
                    let leaf_field = options.levels.last().map(|s| s.as_str()).unwrap_or("");
                    let strata = app.strata_by_resource_id.values().find(|s| {
                        strata_field_value(s, leaf_field)
                            .map(|v| v.trim().to_string())
                            .as_deref() == Some(trimmed)
                    });
                    if let Some(s) = strata {
                        for field in &preset_fields {
                            let val = if *field == "cpuset" {
                                cpuset_display_aggregate(app, leaf_field, trimmed)
                            } else {
                                strata_field_value(s, field)
                                    .map(|v| v.trim().to_string())
                            };
                            tooltip_text.push_str(&format!("{}: {}\n", field, val.as_deref().unwrap_or("")));
                        }
                    }
                }
            }
        }

        tooltip_text.push_str(&format!("{} State: {:?}", kind_label, resource_state));
        options.current_hovered_resource_state = None;
        options.current_hovered_resource_label = None;
    }

    if let Some(iv) = options.current_hovered_dead_interval.take() {
        let state_label = match iv.state {
            ResourceState::Dead => "Dead",
            ResourceState::Absent => "Absent",
            ResourceState::Suspected => "Suspected",
            ResourceState::Standby => "Standby",
            _ => "Unknown",
        };
        const OAR_INDEFINITE: i64 = 2_147_483_646;
        let until_str = if iv.end_s == 0 || iv.end_s >= OAR_INDEFINITE {
            "Indefinite".to_string()
        } else {
            format_timestamp(iv.end_s)
        };
        let since_label = if iv.state == ResourceState::Standby { "Since (now)" } else { "Since" };
        let until_label = "Until";
        let interval_text = format!(
            "{}\n{}: {}\n{}: {}",
            state_label,
            since_label,
            format_timestamp(iv.start_s),
            until_label,
            until_str,
        );
        if !tooltip_text.is_empty() {
            tooltip_text.push('\n');
        }
        tooltip_text.push_str(&interval_text);
    }

    if !tooltip_text.is_empty() {
        if let Some(_) = info.response.hover_pos() {
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

// ---------------------------------------------------------------------------
// PaintedJobRow — collected for cross-row label placement
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PaintedJobRow {
    job_id: u32,
    start_s: i64,
    stop_s: i64,
    row_y: f32,
    row_label: String,
    row_index: usize,
}

struct JobBlock<'a> {
    job_id: u32,
    start_s: i64,
    stop_s: i64,
    rows: Vec<&'a PaintedJobRow>,
}

fn collect_contiguous_job_blocks<'a>(rows: &'a [PaintedJobRow], rect_height: f32) -> Vec<JobBlock<'a>> {
    use std::collections::HashMap;
    let mut group_map: HashMap<(u32, i64, i64), Vec<&PaintedJobRow>> = HashMap::new();
    for row in rows.iter() {
        group_map.entry((row.job_id, row.start_s, row.stop_s)).or_default().push(row);
    }
    let mut blocks = Vec::new();
    for ((job_id, start_s, stop_s), mut group_rows) in group_map {
        group_rows.sort_by(|a, b| a.row_y.partial_cmp(&b.row_y).unwrap_or(std::cmp::Ordering::Equal));
        let mut i = 0;
        while i < group_rows.len() {
            let mut j = i + 1;
            while j < group_rows.len() {
                let gap = group_rows[j].row_y - (group_rows[j - 1].row_y + rect_height);
                if gap > 2.0 {
                    break;
                }
                j += 1;
            }
            blocks.push(JobBlock { job_id, start_s, stop_s, rows: group_rows[i..j].to_vec() });
            i = j;
        }
    }
    blocks
}

pub(super) fn paint_job_id_labels(info: &Info, options: &Options, rows: &[PaintedJobRow]) {
    if rows.is_empty() {
        return;
    }
    let chart_clip_rect = Rect::from_min_max(
        pos2(info.canvas.min.x + info.gutter_width, info.canvas.min.y),
        pos2(info.canvas.max.x, info.canvas.max.y),
    );
    let height = options.rect_height.max(info.text_height);
    let font = FontId::proportional(12.0);
    let blocks = collect_contiguous_job_blocks(rows, height);

    for block in blocks {
        let start_x = info.point_from_s(options, block.start_s);
        let end_x = info.point_from_s(options, block.stop_s);
        let block_width = end_x - start_x;
        if block_width <= 30.0 {
            continue;
        }
        let block_top = block.rows.first().unwrap().row_y;
        let block_bottom = block.rows.last().unwrap().row_y + height;
        let block_rect = Rect::from_min_max(pos2(start_x, block_top), pos2(end_x, block_bottom))
            .intersect(chart_clip_rect);
        if !block_rect.is_negative() {
            info.painter
                .with_clip_rect(block_rect)
                .text(
                    block_rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{}", block.job_id),
                    font.clone(),
                    Color32::BLACK,
                );
        }
    }
}

// ---------------------------------------------------------------------------
// Job-field extraction  (path-based: one path per host, higher fields derived
// from strata so a job never appears under the wrong cluster/group)
// ---------------------------------------------------------------------------

pub(super) fn strata_field_value(s: &Strata, field: &str) -> Option<String> {
    match field {
        "cluster" => s.cluster.clone(),
        "host" => s.network_address.clone().or_else(|| s.host.clone()),
        "network_address" => s.network_address.clone(),
        "ip" => s.ip.clone(),
        "site" => {
            let fqdn = s.network_address.as_deref().or(s.host.as_deref()).unwrap_or("");
            let site = fqdn.split('.').nth(1).unwrap_or("").trim();
            if site.is_empty() { None } else { Some(site.to_string()) }
        }
        "state" => s.state.clone(),
        "next_state" => s.next_state.clone(),
        "cputype" => s.cputype.clone(),
        "nodemodel" => s.nodemodel.clone(),
        "chassis" => s.chassis.clone(),
        "gpu_model" | "gpumodel" => s.gpu_model.clone(),
        "gpu_compute_capability" => s.gpu_compute_capability.clone(),
        "type" => s.r#type.clone(),
        "comment" => s.comment.clone(),
        "production" => s.production.clone(),
        "besteffort" => s.besteffort.clone(),
        "deploy" => s.deploy.clone(),
        "drain" => s.drain.clone(),
        "desktop_computing" => s.desktop_computing.clone(),
        "cpufreq" => s.cpufreq.clone(),
        "memnode" => s.memnode.map(|v| format!("{} MB", v / 1024)),
        "memcore" => s.memcore.map(|v| v.to_string()),
        "memcpu" => s.memcpu.map(|v| format!("{} MB", v / 1024)),
        "core_count" => s.core_count.map(|v| v.to_string()),
        "thread_count" => s.thread_count.map(|v| v.to_string()),
        "resource_id" => s.resource_id.map(|v| v.to_string()),
        // Kavlan
        "vlan" => s.vlan.clone(),
        "subnet_address" => s.subnet_address.clone(),
        "subnet_prefix" => s.subnet_prefix.map(|p| p.to_string()),
        "slash_16" => s.slash_16.clone(),
        "slash_17" => s.slash_17.clone(),
        "slash_18" => s.slash_18.clone(),
        "slash_19" => s.slash_19.clone(),
        "slash_20" => s.slash_20.clone(),
        "slash_21" => s.slash_21.clone(),
        "slash_22" => s.slash_22.clone(),
        // Disk
        "disk" => s.disk.clone(),
        "nodeset" => s.nodeset.clone(),
        // Named Strata fields not yet exposed
        "state_num" => s.state_num.map(|v| v.to_string()),
        "rconsole" => s.rconsole.clone(),
        "cluster_priority" => s.cluster_priority.map(|v| v.to_string()),
        "core" => s.core.map(|v| v.to_string()),
        "suspended_jobs" => s.suspended_jobs.clone(),
        "eth_rate" => s.eth_rate.map(|v| v.to_string()),
        "gpudevice" => s.gpudevice.as_ref().map(|v| v.to_string()),
        // Fall back to the catch-all extra map for any other data.json field
        field => s.extra.get(field).map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        }),
    }
}

fn strata_field_string(strata: &Strata, field: &str) -> String {
    strata_field_value(strata, field)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("(no {})", field))
}

// ---------------------------------------------------------------------------
// ResourceGroup — resource-first recursive hierarchy node
// ---------------------------------------------------------------------------

pub(super) enum ResourceGroup<'a> {
    /// `label` is pre-computed from the view's `leaf_label_template` at build
    /// time (when strata fields are available).  `None` → render the raw key.
    Leaf { jobs: Vec<&'a Job>, label: Option<String> },
    Inner(BTreeMap<String, ResourceGroup<'a>>),
}

// Each assignment carries: path, jobs, pre-computed leaf label.
fn group_resource_paths<'a>(
    assignments: Vec<(Vec<String>, Vec<&'a Job>, Option<String>)>,
    n_levels: usize,
) -> BTreeMap<String, ResourceGroup<'a>> {
    let mut by_key: BTreeMap<String, Vec<(Vec<String>, Vec<&'a Job>, Option<String>)>> = BTreeMap::new();
    for (path, jobs, label) in assignments {
        let key = path[0].clone();
        by_key.entry(key).or_default().push((path[1..].to_vec(), jobs, label));
    }
    by_key.into_iter().map(|(k, sub)| {
        let group = if n_levels == 1 {
            // All entries at this leaf share the same pre-computed label (first wins).
            let label = sub.iter().find_map(|(_, _, l)| l.clone());
            let mut all_jobs: Vec<&'a Job> = sub.into_iter().flat_map(|(_, j, _)| j).collect();
            all_jobs.sort_by_key(|j| j.id);
            all_jobs.dedup_by_key(|j| j.id);
            ResourceGroup::Leaf { jobs: all_jobs, label }
        } else {
            ResourceGroup::Inner(group_resource_paths(sub, n_levels - 1))
        };
        (k, group)
    }).collect()
}

fn min_leaf_label<'a>(group: &'a ResourceGroup<'_>) -> Option<&'a str> {
    match group {
        ResourceGroup::Leaf { label, .. } => label.as_deref(),
        ResourceGroup::Inner(sub) => sub.values().find_map(min_leaf_label),
    }
}

/// Resolve a single field value for a resource, with sibling-lookup enrichment.
///
/// Non-compute resource types (disks, kavlan, …) often lack `cluster`, `site`,
/// or even `host` as direct fields.  For those we find the sibling compute node
/// in `strata_by_host` (linked via `network_address`, `host`, or `nodeset`) and
/// copy the missing value from there.
pub(super) fn resolve_field(
    strata: &Strata,
    field: &str,
    strata_by_host: &std::collections::HashMap<String, Strata>,
) -> String {
    // Find the sibling compute-node strata via any host-linking field on the resource.
    let sibling = || -> Option<&Strata> {
        for key in [
            strata.network_address.as_deref(),
            strata.host.as_deref(),
            strata.nodeset.as_deref(),
        ].iter().flatten() {
            let key = key.trim();
            if key.is_empty() { continue; }
            if let Some(h) = strata_by_host.get(key)
                .or_else(|| strata_by_host.get(key.split('.').next().unwrap_or(key)))
            {
                return Some(h);
            }
        }
        None
    };

    match field {
        "host" | "network_address" => {
            // Own fields first, then sibling's network_address for normalization.
            strata.network_address.as_deref()
                .or(strata.host.as_deref())
                .or(strata.nodeset.as_deref())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| sibling()?.network_address.as_deref()
                    .map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
                .unwrap_or_else(|| format!("(no {})", field))
        }

        "cluster" => {
            // Own field → sibling compute node's cluster.
            strata.cluster.as_deref().map(str::trim).filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| sibling()?.cluster.as_deref()
                    .map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
                .unwrap_or_else(|| "(no cluster)".to_string())
        }

        "site" => {
            // Derive from own FQDN first, then from sibling's FQDN.
            let site_from = |fqdn: &str| -> Option<String> {
                let s = fqdn.split('.').nth(1).unwrap_or("").trim();
                if s.is_empty() { None } else { Some(s.to_string()) }
            };
            let own_fqdn = strata.network_address.as_deref()
                .or(strata.host.as_deref())
                .or(strata.nodeset.as_deref())
                .unwrap_or("");
            site_from(own_fqdn)
                .or_else(|| {
                    let sib = sibling()?;
                    let fqdn = sib.network_address.as_deref()
                        .or(sib.host.as_deref())
                        .unwrap_or("");
                    site_from(fqdn)
                })
                .or_else(|| {
                    // kavlan/subnet resources have no FQDN at all; pick from any
                    // compute node since OAR data is always single-site.
                    strata_by_host.values().find_map(|s| {
                        let fqdn = s.network_address.as_deref()
                            .or(s.host.as_deref())
                            .unwrap_or("");
                        site_from(fqdn)
                    })
                })
                .unwrap_or_else(|| "(no site)".to_string())
        }

        _ => strata_field_string(strata, field),
    }
}

/// Returns the longest leaf label string that will be rendered for the current view.
/// Used by `compute_gutter_width` so the gutter fits its content exactly.
pub(super) fn max_leaf_label(
    app: &ApplicationContext,
    levels: &[String],
    template: Option<&str>,
) -> String {
    let leaf_field = match levels.last() {
        Some(f) => f.as_str(),
        None => return String::new(),
    };
    let ancestor_fields = &levels[..levels.len().saturating_sub(1)];
    let mut longest = String::new();
    for strata in app.strata_by_resource_id.values() {
        let raw = resolve_field(strata, leaf_field, &app.strata_by_host);
        if raw.starts_with("(no ") {
            continue;
        }
        let label = if let Some(tmpl) = template {
            let path_context: Vec<(String, String)> = ancestor_fields
                .iter()
                .map(|f| (f.clone(), resolve_field(strata, f, &app.strata_by_host)))
                .collect();
            resolve_label_with_strata(tmpl, &path_context, leaf_field, &raw, strata)
        } else {
            raw
        };
        if label.len() > longest.len() {
            longest = label;
        }
    }
    longest
}

/// Build a resource-first group tree.
/// Every resource in `strata_by_resource_id` whose LEAF-level field is present
/// is included as a row, even with zero jobs.  Missing intermediate fields are
/// derived where possible (e.g. cluster inferred from host lookup).
/// Jobs are overlaid as a reverse lookup: resource_id → jobs.
pub(super) fn build_resource_groups<'a>(
    app: &'a ApplicationContext,
    levels: &[String],
    jobs: &[&'a Job],
    filter: Option<&super::types::ResourceFilter>,
    template: Option<&str>,
) -> BTreeMap<String, ResourceGroup<'a>> {
    use std::collections::{HashMap, HashSet};
    assert!(!levels.is_empty(), "levels must not be empty");

    let mut jobs_by_resource: HashMap<u32, Vec<&'a Job>> = HashMap::new();
    for &job in jobs {
        for &rid in &job.assigned_resources {
            jobs_by_resource.entry(rid).or_default().push(job);
        }
    }

    let job_by_id: HashMap<u32, &'a Job> = jobs.iter().map(|&j| (j.id, j)).collect();

    // path -> (job id set, representative strata for label resolution)
    let mut leaf_data: BTreeMap<Vec<String>, (HashSet<u32>, &Strata)> = BTreeMap::new();

    for (&rid, strata) in &app.strata_by_resource_id {
        let path: Vec<String> = levels.iter()
            .map(|f| resolve_field(strata, f, &app.strata_by_host))
            .collect();
        if path.last().map_or(true, |v| v.starts_with("(no ")) {
            continue;
        }
        if let Some(f) = filter {
            let actual = strata_field_value(strata, &f.field).unwrap_or_default();
            let matches = actual.trim() == f.value.trim();
            if f.exclude == matches {
                continue;
            }
        }
        let entry = leaf_data.entry(path).or_insert_with(|| (HashSet::new(), strata));
        if let Some(resource_jobs) = jobs_by_resource.get(&rid) {
            for job in resource_jobs {
                entry.0.insert(job.id);
            }
        }
    }

    let assignments: Vec<(Vec<String>, Vec<&'a Job>, Option<String>)> = leaf_data
        .into_iter()
        .map(|(path, (job_ids, rep_strata))| {
            let mut job_list: Vec<&'a Job> = job_ids.iter()
                .filter_map(|id| job_by_id.get(id).copied())
                .collect();
            job_list.sort_by_key(|j| j.id);

            let label = template.map(|tmpl| {
                // Build context: ancestor levels + all strata fields as fallback.
                let path_context: Vec<(String, String)> = levels[..levels.len() - 1]
                    .iter()
                    .zip(path.iter())
                    .map(|(f, v)| (f.clone(), v.clone()))
                    .collect();
                let leaf_field = levels.last().map(|s| s.as_str()).unwrap_or("");
                let leaf_key = path.last().map(|s| s.as_str()).unwrap_or("");
                // resolve_label: path_context first, then raw strata field.
                resolve_label_with_strata(tmpl, &path_context, leaf_field, leaf_key, rep_strata)
            });

            (path, job_list, label)
        })
        .collect();

    group_resource_paths(assignments, levels.len())
}

/// Like `apply_label_template` but falls back to `strata_field_value` for
/// placeholders not found in path_context or the leaf key.
fn resolve_label_with_strata(
    template: &str,
    path_context: &[(String, String)],
    leaf_field: &str,
    leaf_key: &str,
    strata: &Strata,
) -> String {
    let lookup = |field: &str| -> String {
        if field == leaf_field { return leaf_key.to_string(); }
        if let Some((_, v)) = path_context.iter().find(|(f, _)| f == field) {
            return v.clone();
        }
        strata_field_value(strata, field).unwrap_or_default()
    };

    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        result.push_str(&rest[..open]);
        rest = &rest[open + 1..];
        if let Some(close) = rest.find('}') {
            let inner = &rest[..close];
            rest = &rest[close + 1..];
            let rendered = if let Some(pipe) = inner.find('|') {
                let field = &inner[..pipe];
                let modifier = &inner[pipe + 1..];
                let raw = lookup(field);
                match modifier {
                    "short" => raw.split('.').next().unwrap_or(&raw).to_string(),
                    _ => raw,
                }
            } else {
                lookup(inner)
            };
            result.push_str(&rendered);
        } else {
            result.push('{');
            result.push_str(rest);
            break;
        }
    }
    result.push_str(rest);
    result
}

// ---------------------------------------------------------------------------
// Stripe colour per depth (outermost = index 0 = lightest)
// ---------------------------------------------------------------------------

fn stripe_color(depth: usize) -> Color32 {
    const PALETTE: [(u8, u8, u8); 3] = [
        (252, 238, 170), // outermost – lightest yellow
        (245, 227, 113), // middle
        (235, 215, 110), // innermost – darkest yellow
    ];
    let (r, g, b) = PALETTE[depth % PALETTE.len()];
    Color32::from_rgb(r, g, b)
}

// ---------------------------------------------------------------------------
// Leaf-level label (drawn to the LEFT of the stripe)
// ---------------------------------------------------------------------------

/// Resolve a `{field}` / `{field|modifier}` template.
///
/// Supported modifiers:
/// - `short` — take the first segment before `.` (strips domain from FQDNs)
///
/// Unknown `{placeholders}` are left unchanged.
fn apply_label_template(
    template: &str,
    path_context: &[(String, String)],
    leaf_field: &str,
    leaf_key: &str,
) -> String {
    let lookup = |field: &str| -> String {
        if field == leaf_field {
            return leaf_key.to_string();
        }
        path_context.iter()
            .find(|(f, _)| f == field)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        result.push_str(&rest[..open]);
        rest = &rest[open + 1..];
        if let Some(close) = rest.find('}') {
            let inner = &rest[..close];
            rest = &rest[close + 1..];
            let rendered = if let Some(pipe) = inner.find('|') {
                let field = &inner[..pipe];
                let modifier = &inner[pipe + 1..];
                let raw = lookup(field);
                match modifier {
                    "short" => raw.split('.').next().unwrap_or(&raw).to_string(),
                    _ => raw,
                }
            } else {
                lookup(inner)
            };
            result.push_str(&rendered);
        } else {
            result.push('{');
            result.push_str(rest);
            break;
        }
    }
    result.push_str(rest);
    result
}

fn draw_leaf_label(
    info: &Info,
    key: &str,
    precomputed_label: Option<&str>,
    row_y: f32,
    label_x_end: f32,
    row_height: f32,
    app: &ApplicationContext,
    options: &Options,
    path_context: &[(String, String)],
) {
    let theme_colors = get_theme_colors(&info.ctx.style());
    let visuals = info.ctx.style().visuals.clone();

    let label_text = precomputed_label.unwrap_or(key).to_string();

    let indent = 2.0;
    let left = info.canvas.min.x + indent;
    let total_width = (label_x_end - indent).max(40.0);
    let rect = Rect::from_min_max(
        pos2(left, row_y),
        pos2(left + total_width, row_y + row_height),
    );

    let is_hovered = info.response.hover_pos().map_or(false, |m| rect.contains(m));

    let clip_rect = Rect::from_min_max(
        pos2(info.canvas.min.x, row_y),
        pos2(info.canvas.min.x + label_x_end, row_y + row_height),
    );
    let gutter_painter = info.painter.with_clip_rect(clip_rect);

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
        Stroke::new(2.0, visuals.selection.stroke.color)
    } else {
        label_border
    };
    gutter_painter.rect(rect, 0.0, label_bg, border);

    if is_hovered {
        let hover_fill = Color32::from_rgba_unmultiplied(120, 120, 120, 140);
        gutter_painter.rect_filled(
            Rect::from_min_max(pos2(info.canvas.min.x, row_y), pos2(info.canvas.min.x + 4.0, row_y + row_height)),
            0.0,
            hover_fill,
        );
    }

    let label_font = FontId::proportional((info.font_id.size - 1.0).max(11.0));
    gutter_painter.text(
        pos2(left + 6.0, row_y + row_height * 0.5),
        Align2::LEFT_CENTER,
        &label_text,
        label_font.clone(),
        if is_hovered { theme_colors.text } else { label_text_color },
    );

    if is_hovered {
        info.ctx.set_cursor_icon(CursorIcon::PointingHand);
        let preset_fields = options.leaf_info_preset.as_ref()
            .map(|p| p.fields.as_slice())
            .unwrap_or(&[]);
        if !preset_fields.is_empty() {
            let mut fields_snapshot: Vec<String> = preset_fields.iter().cloned().collect();
            fields_snapshot.sort_unstable();
            let layer_id = LayerId::new(Order::Tooltip, Id::new("gantt-label-layer"));
            egui::containers::popup::show_tooltip(
                &info.ctx,
                layer_id,
                Id::new(format!("gantt-leaf-label-{}", key)),
                |ui: &mut egui::Ui| {
                    let trimmed = key.trim();
                    let leaf_field = options.levels.last().map(|s| s.as_str()).unwrap_or("");
                    let strata_label = app.strata_by_resource_id.values().find(|s| {
                        strata_field_value(s, leaf_field)
                            .map(|v| v.trim().to_string())
                            .as_deref() == Some(trimmed)
                    });
                    if let Some(s) = strata_label {
                        for field in &fields_snapshot {
                            let val = if field == "cpuset" {
                                cpuset_display_aggregate(app, leaf_field, trimmed)
                            } else {
                                strata_field_value(s, field)
                                    .map(|v| v.trim().to_string())
                            };
                            ui.label(format!("{}: {}", field, val.as_deref().unwrap_or("")));
                        }
                    }
                },
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Resource-state lookup for any field
// ---------------------------------------------------------------------------

fn resource_ids_for_leaf(
    field: &str,
    key: &str,
    path_context: &[(String, String)],
    strata_by_resource_id: &HashMap<u32, Strata>,
) -> Vec<u32> {
    strata_by_resource_id
        .values()
        .filter(|s| {
            strata_field_value(s, field).as_deref() == Some(key)
                && path_context
                    .iter()
                    .all(|(f, v)| strata_field_value(s, f).as_deref() == Some(v.as_str()))
        })
        .filter_map(|s| s.resource_id)
        .collect()
}

fn draw_dead_intervals_for_row(
    info: &Info,
    options: &mut Options,
    top_y: f32,
    height: f32,
    intervals: &[DeadInterval],
    app: &ApplicationContext,
) {
    if intervals.is_empty() {
        return;
    }
    let sc = &app.gantt_config.state_colors;
    let min_dur = app.gantt_config.min_state_duration_s;
    let chart_clip_rect = Rect::from_min_max(
        pos2(info.canvas.min.x + info.gutter_width, info.canvas.min.y),
        pos2(info.canvas.max.x, info.canvas.max.y),
    );
    let chart_painter = info.painter.with_clip_rect(chart_clip_rect);
    let hachure_spacing = 10.0;

    for iv in intervals {
        // Skip intervals shorter than min_state_duration_s (closed intervals only).
        if iv.end_s != 0 && min_dur > 0 && (iv.end_s - iv.start_s) < min_dur {
            continue;
        }
        let mk = |r: u8, g: u8, b: u8| Color32::from_rgba_premultiplied(r, g, b, 150);
        let base_color = match iv.state {
            ResourceState::Dead     => mk(sc.dead.0,     sc.dead.1,     sc.dead.2),
            ResourceState::Absent   => mk(sc.absent.0,   sc.absent.1,   sc.absent.2),
            ResourceState::Suspected => mk(sc.suspected.0, sc.suspected.1, sc.suspected.2),
            ResourceState::Standby  => Color32::from_rgb(sc.standby.0, sc.standby.1, sc.standby.2).gamma_multiply(0.7),
            _ => continue,
        };
        let start_x = info.point_from_s(options, iv.start_s);
        let end_x = if iv.end_s == 0 {
            info.canvas.max.x
        } else {
            info.point_from_s(options, iv.end_s)
        };

        let hover_rect = Rect::from_min_max(
            pos2(start_x.max(info.canvas.min.x + info.gutter_width), top_y),
            pos2(end_x.min(info.canvas.max.x), top_y + height),
        );
        let is_hovered = info.response.hover_pos().map_or(false, |m| hover_rect.contains(m));
        if is_hovered && options.current_hovered_dead_interval.is_none() {
            options.current_hovered_dead_interval = Some(iv.clone());
        }
        let color = if is_hovered { base_color.gamma_multiply(1.5) } else { base_color };

        let mut x = start_x.max(info.canvas.min.x);
        let mut shapes = Vec::new();
        while x < end_x.min(info.canvas.max.x) {
            shapes.push(Shape::line_segment(
                [pos2(x, top_y), pos2(x + hachure_spacing, top_y + height)],
                Stroke::new(2.0, color),
            ));
            x += hachure_spacing;
        }
        chart_painter.extend(shapes);
    }
}

// ---------------------------------------------------------------------------
// Recursive draw_level_n
// ---------------------------------------------------------------------------

/// Draw `n` hierarchy levels recursively.
///
/// * **n == 1** (leaf): draws a coloured panel separated by a line; the item
///   label is rendered to the LEFT of the panel.
/// * **n > 1** (inner): draws the current panel (yellow band + dividing line,
///   no label) then recurses with `n − 1`.
///
/// Returns the updated `cursor_y` after all rows have been painted.
pub(super) fn draw_level_n<'a>(
    info: &Info,
    options: &mut Options,
    groups: &BTreeMap<String, ResourceGroup<'a>>,
    levels: &[String],
    depth: usize,
    n_total: usize,
    stripe_x: f32,
    label_x_end: f32,
    gutter_width: f32,
    mut cursor_y: f32,
    details_window: &mut Vec<JobDetailsWindow>,
    app: &ApplicationContext,
    all_clusters: &Vec<Cluster>,
    painted_rows: &mut Vec<PaintedJobRow>,
    // Accumulated (field, value) pairs from ancestor levels, used for stripe tooltip.
    path_context: &[(String, String)],
) -> f32 {
    let theme_colors = get_theme_colors(&info.ctx.style());
    let chart_x0 = info.canvas.min.x + gutter_width;
    let row_height = options.rect_height.max(info.text_height);
    let s_color = stripe_color(depth);
    let stripe_border = Stroke::new(1.0, Color32::BLACK);
    let field = &levels[0];

    // Outer-level dividers are thicker and lighter; inner ones thinner.
    let line_stroke = if depth == 0 {
        Stroke::new(1.5, Color32::from(theme_colors.aggregated_line_level_1))
    } else {
        Stroke::new(0.5, Color32::from(theme_colors.aggregated_line_level_2))
    };

    let mut sorted_keys: Vec<&String> = groups.keys().collect();
    if options.sort_by_label {
        sorted_keys.sort_by(|a, b| {
            let la = min_leaf_label(groups.get(*a).unwrap())
                .map(str::to_string)
                .unwrap_or_else(|| (*a).clone());
            let lb = min_leaf_label(groups.get(*b).unwrap())
                .map(str::to_string)
                .unwrap_or_else(|| (*b).clone());
            compare_string_with_number(&la, &lb)
        });
    } else {
        sorted_keys.sort_by(|a, b| compare_string_with_number(a, b));
    }

    for key in sorted_keys {
        let group = groups.get(key).unwrap();
        let span_top = cursor_y;

        // Horizontal dividing line across the chart area.
        info.painter.line_segment(
            [pos2(chart_x0, cursor_y), pos2(info.canvas.max.x, cursor_y)],
            line_stroke,
        );

        match group {
            // ── n == 1: leaf ─────────────────────────────────────────────────
            ResourceGroup::Leaf { jobs, label } => {
                draw_leaf_label(info, key, label.as_deref(), cursor_y, label_x_end, row_height, app, options, path_context);

                let job_row_y = cursor_y;

                // Check if hovering over the entire row and draw vertical dash
                let row_rect = Rect::from_min_max(
                    pos2(info.canvas.min.x, job_row_y),
                    pos2(info.canvas.max.x, job_row_y + row_height),
                );
                let is_row_hovered = info.response.hover_pos()
                    .map_or(false, |m| row_rect.contains(m));
                if is_row_hovered {
                    let hover_fill = Color32::from_rgba_unmultiplied(120, 120, 120, 140);
                    info.painter.rect_filled(
                        Rect::from_min_max(
                            pos2(info.canvas.min.x, job_row_y),
                            pos2(info.canvas.min.x + 4.0, job_row_y + row_height),
                        ),
                        0.0,
                        hover_fill,
                    );
                }

                for job in jobs.iter() {
                    paint_job(
                        info,
                        options,
                        job,
                        job_row_y,
                        details_window,
                        all_clusters,
                        Some(key.as_str()),
                        app.gantt_config.besteffort_truncate_to_now,
                    );
                }

                // Draw dead/absent/suspected/standby intervals for this row.
                let resource_ids = resource_ids_for_leaf(field, key, path_context, &app.strata_by_resource_id);
                let mut seen: HashSet<DeadInterval> = HashSet::new();
                let mut intervals: Vec<DeadInterval> = Vec::new();
                for id in &resource_ids {
                    if let Some(ivs) = app.dead_intervals.get(id) {
                        for iv in ivs {
                            if seen.insert(iv.clone()) {
                                intervals.push(iv.clone());
                            }
                        }
                    }
                }

                // Standby: mutate open-ended Absent intervals in place if resource has future available_upto.
                let now_s = chrono::Utc::now().timestamp();
                let has_standby = resource_ids.iter().any(|id| app.standby_upto.contains_key(id));
                if has_standby {
                    for iv in intervals.iter_mut() {
                        if iv.state == ResourceState::Absent && iv.end_s == 0 {
                            iv.state = ResourceState::Standby;
                            if app.gantt_config.standby_truncate_to_now {
                                iv.end_s = now_s;
                            }
                        }
                    }
                }

                draw_dead_intervals_for_row(info, options, job_row_y, row_height, &intervals, app);


                // Collect rows for job-ID label painting.
                let mut sorted_jobs: Vec<&&Job> = jobs.iter().collect();
                sorted_jobs.sort_by_key(|j| (j.scheduled_start, j.stop_time));
                for job in sorted_jobs {
                    let stop_s = if job.stop_time > 0 {
                        job.stop_time
                    } else {
                        job.scheduled_start + job.walltime
                    };
                    painted_rows.push(PaintedJobRow {
                        job_id: job.id,
                        start_s: job.scheduled_start,
                        stop_s,
                        row_y: job_row_y,
                        row_label: key.clone(),
                        row_index: painted_rows.len(),
                    });
                }

                cursor_y += row_height;
            }

            // ── n > 1: inner ─────────────────────────────────────────────────
            ResourceGroup::Inner(sub_groups) => {
                let mut child_context = path_context.to_vec();
                child_context.push((field.clone(), key.clone()));

                cursor_y = draw_level_n(
                    info,
                    options,
                    sub_groups,
                    &levels[1..],
                    depth + 1,
                    n_total,
                    stripe_x + STRIPE_W,
                    label_x_end,
                    gutter_width,
                    cursor_y,
                    details_window,
                    app,
                    all_clusters,
                    painted_rows,
                    &child_context,
                );
            }
        }

        let span_bottom = cursor_y;

        // Draw this level's coloured stripe spanning the full group extent.
        if span_bottom > span_top {
            let stripe_rect = Rect::from_min_max(
                pos2(stripe_x, span_top),
                pos2(stripe_x + STRIPE_W, span_bottom),
            );

            let is_hovered = info.response.hover_pos()
                .map_or(false, |m| stripe_rect.contains(m));

            let fill = if is_hovered {
                Color32::from_rgba_unmultiplied(100, 149, 237, 180) // cornflower-blue highlight
            } else {
                s_color
            };
            info.painter.rect_filled(stripe_rect, 0.0, fill);

            // Stripe hover tooltip — shows the full path from root to this level.
            if is_hovered {
                let mut tooltip_lines: Vec<String> = path_context
                    .iter()
                    .map(|(f, v)| format!("{}: {}", f, v))
                    .collect();
                tooltip_lines.push(format!("{}: {}", field, key));
                let tooltip_text = tooltip_lines.join("\n");
                egui::show_tooltip_at_pointer(
                    &info.ctx,
                    info.response.layer_id,
                    egui::Id::new(("stripe_tip", depth, key.as_str())),
                    |ui| { ui.label(tooltip_text); },
                );
            }

            // Horizontal separator at the bottom of each group within the stripe.
            if span_bottom < info.canvas.max.y {
                info.painter.line_segment(
                    [pos2(stripe_x, span_bottom), pos2(stripe_x + STRIPE_W, span_bottom)],
                    stripe_border,
                );
            }
        }
    }

    cursor_y
}

/// Draw the outer border rect and vertical dividers between stripe columns.
/// Called once from `canvas.rs` after `draw_level_n` finishes.
pub(super) fn draw_stripe_borders(
    info: &Info,
    n_total: usize,
    gutter_width: f32,
    y_top: f32,
    y_bottom: f32,
) {
    if n_total == 0 || y_bottom <= y_top {
        return;
    }
    let inset = 0.5;
    let border = Stroke::new(1.0, Color32::BLACK);
    let stripes_x0 = info.canvas.min.x + gutter_width - gutter_stripes_total_w(n_total);
    let stripes_x1 = info.canvas.min.x + gutter_width;

    // Outer rect
    info.painter.rect_stroke(
        Rect::from_min_max(
            pos2(stripes_x0 + inset, y_top + inset),
            pos2(stripes_x1 - inset, y_bottom - inset),
        ),
        0.0,
        border,
    );

    // Vertical dividers between stripes
    for i in 1..n_total {
        let x = stripes_x0 + i as f32 * STRIPE_W;
        info.painter.line_segment(
            [pos2(x, y_top + inset), pos2(x, y_bottom - inset)],
            border,
        );
    }

    // Thick right border (matching Grid5000 style)
    let right_border = Stroke::new(2.0, Color32::BLACK);
    info.painter.line_segment(
        [pos2(stripes_x1 - 1.0, y_top + inset), pos2(stripes_x1 - 1.0, y_bottom - inset)],
        right_border,
    );
}

// ---------------------------------------------------------------------------
// paint_job — unchanged from original
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum PaintResult {
    Culled,
    Painted,
}

fn paint_job(
    info: &Info,
    options: &mut Options,
    job: &Job,
    top_y: f32,
    details_window: &mut Vec<JobDetailsWindow>,
    all_cluster: &Vec<Cluster>,
    resource_label_for_state_tooltip: Option<&str>,
    besteffort_truncate_to_now: bool,
) -> PaintResult {
    let chart_clip_rect = Rect::from_min_max(
        pos2(info.canvas.min.x + info.gutter_width, info.canvas.min.y),
        pos2(info.canvas.max.x, info.canvas.max.y),
    );
    let chart_painter = info.painter.with_clip_rect(chart_clip_rect);
    let start_x = info.point_from_s(options, job.scheduled_start);
    let raw_stop = if job.stop_time > 0 { job.stop_time } else { job.scheduled_start + job.walltime };
    let stop_time = if besteffort_truncate_to_now
        && job.job_types.iter().any(|t| t == "besteffort")
        && raw_stop > chrono::Utc::now().timestamp()
    {
        chrono::Utc::now().timestamp()
    } else {
        raw_stop
    };
    let end_x = info.point_from_s(options, stop_time);
    let width = end_x - start_x;

    if width < options.cull_width {
        return PaintResult::Culled;
    }

    let height = options.rect_height.max(info.text_height);
    let rounding = options.rounding;
    let rect = Rect::from_min_size(pos2(start_x, top_y), egui::vec2(width.max(options.min_width), height));
    let visible_rect = rect.intersect(chart_clip_rect);
    if visible_rect.is_negative() {
        return PaintResult::Culled;
    }

    let is_job_trully_hovered = info.response.hover_pos().map_or(false, |m| visible_rect.contains(m));

    // Update hovered leaf label for cross-highlighting.
    if is_job_trully_hovered {
        if let Some(label) = resource_label_for_state_tooltip {
            let trimmed = label.trim();
            if !trimmed.is_empty() {
                options.hovered_leaf_label = Some(trimmed.to_string());
            }
        }
    }

    let is_job_hovered = is_job_trully_hovered
        || options.current_hovered_job.as_ref().map_or(false, |j| j.id == job.id)
        || options.previous_hovered_job.as_ref().map_or(false, |j| j.id == job.id);

    if is_job_trully_hovered && options.current_hovered_job.is_none() {
        options.current_hovered_job = Some(job.clone());
    }

    if is_job_trully_hovered && info.response.secondary_clicked() {
        let window = JobDetailsWindow::new(job.clone(), get_tree_structure_for_job(job, all_cluster));
        if !details_window.iter().any(|w| w.job.id == job.id) {
            details_window.push(window);
        }
    }

    if is_job_trully_hovered && info.response.clicked() && !info.response.double_clicked() {
        let job_start_s = job.scheduled_start as f64;
        let job_end_s = if job.stop_time > 0 { job.stop_time as f64 } else { job_start_s + job.walltime as f64 };
        options.zoom_to_relative_s_range = Some((
            info.ctx.input(|i| i.time),
            (job_start_s - info.start_s as f64, job_end_s - info.start_s as f64),
        ));
    }

    let (hovered_color, normal_color) = if options.job_color.is_random() {
        job.get_gantt_color()
    } else {
        job.state.get_color()
    };
    let fill_color = if is_job_hovered { hovered_color } else { normal_color };
    chart_painter.rect_filled(visible_rect, rounding, fill_color);

    if is_job_hovered {
        let hover_fill = Color32::from_rgba_unmultiplied(140, 140, 140, 60);
        let hover_stroke = Stroke::new(3.0, Color32::from_gray(80));
        chart_painter.rect_filled(visible_rect, rounding, hover_fill);
        chart_painter.rect_stroke(visible_rect.expand(1.0), rounding, hover_stroke);
    }

    PaintResult::Painted
}

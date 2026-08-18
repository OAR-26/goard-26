use crate::models::data_structure::job::Job;
use crate::models::data_structure::resource::ResourceState;
use crate::models::data_structure::strata::Strata;
use std::collections::HashMap;
use std::hash::DefaultHasher;
use std::hash::Hash;
//
use std::cmp::Ordering;
use std::hash::Hasher;

/// Convert a job ID to a color (using hash).
/// `min` (0–255) is the minimum RGB component, ensuring colors stay light enough
/// for black job-ID labels to remain readable. Configured via `job_color_min` in config.json.
pub fn convert_id_to_color(id: u32, min: u8) -> egui::Color32 {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();

    let range = 255u16.saturating_sub(min as u16);
    let lighten = |byte: u8| -> u8 { min.saturating_add((byte as u16 * range / 255) as u8) };

    let r = lighten(((hash >> 16) & 0xFF) as u8);
    let g = lighten(((hash >> 8) & 0xFF) as u8);
    let b = lighten((hash & 0xFF) as u8);

    egui::Color32::from_rgb(r, g, b)
}

/// Returns `(base_color, darker_color)` for a string value (field-based coloring).
/// Same visual contract as `get_job_gantt_colors`: deterministic, respects `job_color_min`.
pub fn get_job_gantt_colors_for_str(value: &str, job_color_min: u8) -> (egui::Color32, egui::Color32) {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    let seed = (hasher.finish() as u32) | 1; // ensure non-zero so we never get transparent
    get_job_gantt_colors(seed, job_color_min)
}

/// Returns `(base_color, darker_color)` for rendering a job bar.
/// Job id 0 (synthetic all_resources sentinel) → transparent pair.
/// `job_color_min`: minimum RGB component for random colors (from GanttConfig).
pub fn get_job_gantt_colors(job_id: u32, job_color_min: u8) -> (egui::Color32, egui::Color32) {
    if job_id == 0 {
        return (egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT);
    }
    let base = convert_id_to_color(job_id, job_color_min);
    let darker = |c: u8| -> u8 { ((c as f32 * 0.8) as u8).max(1) };
    let d = egui::Color32::from_rgb(darker(base.r()), darker(base.g()), darker(base.b()));
    (base, d)
}

/// All distinct cluster names from strata.
pub fn cluster_names(strata_by_resource_id: &HashMap<u32, Strata>) -> Vec<String> {
    let mut names: Vec<String> = strata_by_resource_id
        .values()
        .filter_map(|s| s.cluster.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    names.sort();
    names
}

/// All distinct host names from strata.
pub fn host_names(strata_by_resource_id: &HashMap<u32, Strata>) -> Vec<String> {
    let mut names: Vec<String> = strata_by_resource_id
        .values()
        .filter_map(|s| s.host.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    names.sort();
    names
}

/// All resource IDs.
pub fn all_resource_ids(strata_by_resource_id: &HashMap<u32, Strata>) -> Vec<u32> {
    strata_by_resource_id.keys().cloned().collect()
}

/// Cluster names the job touches (from its assigned_resources → strata).
pub fn clusters_for_job(job: &Job, strata_by_resource_id: &HashMap<u32, Strata>) -> Vec<String> {
    let mut result = Vec::new();
    for rid in &job.assigned_resources {
        if let Some(s) = strata_by_resource_id.get(rid) {
            if let Some(c) = &s.cluster {
                if !result.contains(c) {
                    result.push(c.clone());
                }
            }
        }
    }
    result
}

/// Host names the job touches.
pub fn hosts_for_job(job: &Job, strata_by_resource_id: &HashMap<u32, Strata>) -> Vec<String> {
    let mut result = Vec::new();
    for rid in &job.assigned_resources {
        if let Some(s) = strata_by_resource_id.get(rid) {
            if let Some(h) = &s.host {
                if !result.contains(h) {
                    result.push(h.clone());
                }
            }
        }
    }
    result
}

/// Aggregate ResourceState for a cluster (worst-case of all its resources).
pub fn cluster_resource_state(
    cluster_name: &str,
    cluster_resource_ids: &HashMap<String, Vec<u32>>,
    strata_by_resource_id: &HashMap<u32, Strata>,
) -> ResourceState {
    let Some(rids) = cluster_resource_ids.get(cluster_name) else {
        return ResourceState::Unknown;
    };
    worst_state(
        rids.iter()
            .filter_map(|rid| strata_by_resource_id.get(rid))
            .filter_map(|s| parse_resource_state(s.state.as_deref())),
    )
}

/// Aggregate ResourceState for a host.
pub fn host_resource_state(
    host_name: &str,
    host_resource_ids: &HashMap<String, Vec<u32>>,
    strata_by_resource_id: &HashMap<u32, Strata>,
) -> ResourceState {
    let Some(rids) = host_resource_ids.get(host_name) else {
        return ResourceState::Unknown;
    };
    worst_state(
        rids.iter()
            .filter_map(|rid| strata_by_resource_id.get(rid))
            .filter_map(|s| parse_resource_state(s.state.as_deref())),
    )
}

fn parse_resource_state(s: Option<&str>) -> Option<ResourceState> {
    match s? {
        "Alive" => Some(ResourceState::Alive),
        "Dead" => Some(ResourceState::Dead),
        "Absent" => Some(ResourceState::Absent),
        "Suspected" => Some(ResourceState::Suspected),
        _ => Some(ResourceState::Unknown),
    }
}

fn worst_state(states: impl Iterator<Item = ResourceState>) -> ResourceState {
    let mut worst = ResourceState::Unknown;
    for s in states {
        match s {
            ResourceState::Dead => return ResourceState::Dead,
            ResourceState::Suspected if !matches!(worst, ResourceState::Dead) => {
                worst = ResourceState::Suspected
            }
            ResourceState::Absent
                if matches!(worst, ResourceState::Unknown | ResourceState::Alive) =>
            {
                worst = ResourceState::Absent
            }
            ResourceState::Alive if matches!(worst, ResourceState::Unknown) => {
                worst = ResourceState::Alive
            }
            _ => {}
        }
    }
    worst
}

// Compare two strings that may contain numbers (natural sort)
pub fn compare_string_with_number(a: &str, b: &str) -> Ordering {
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Token {
        Str(String),
        Num(u64),
    }

    fn tokenize(s: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut chars = s.chars().peekable();

        while let Some(&ch) = chars.peek() {
            if ch.is_ascii_digit() {
                let mut digits = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        digits.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let value = digits.parse::<u64>().unwrap_or(0);
                tokens.push(Token::Num(value));
            } else {
                let mut text = String::new();
                while let Some(&t) = chars.peek() {
                    if !t.is_ascii_digit() {
                        text.push(t);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Str(text.to_lowercase()));
            }
        }

        tokens
    }

    let ta = tokenize(a);
    let tb = tokenize(b);

    let mut i = 0;
    loop {
        let xa = ta.get(i);
        let xb = tb.get(i);

        match (xa, xb) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(Token::Num(na)), Some(Token::Num(nb))) => {
                let ord = na.cmp(nb);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(Token::Str(sa)), Some(Token::Str(sb))) => {
                let ord = sa.cmp(sb);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(Token::Num(_)), Some(Token::Str(_))) => return Ordering::Greater,
            (Some(Token::Str(_)), Some(Token::Num(_))) => return Ordering::Less,
        }

        i += 1;
    }
}

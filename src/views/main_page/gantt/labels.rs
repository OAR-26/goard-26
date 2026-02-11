use crate::models::data_structure::cluster::Cluster;
use crate::views::components::gantt_aggregate_by::{AggregateByLevel1Enum, AggregateByLevel2Enum};

pub(super) struct LabelMeta {
    pub(super) host: Option<String>,
}

pub(super) fn short_host_label(host: &str) -> String {
    let first = host.split('.').next().unwrap_or(host).trim();

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

fn site_from_fqdn(host: &str) -> Option<String> {
    let mut parts = host.split('.');
    let _hostname = parts.next();
    let site = parts.next();
    site.filter(|s| !s.is_empty()).map(|s| s.to_string())
}

pub(super) fn site_for_cluster_name(cluster_name: &str, clusters: &[Cluster]) -> Option<String> {
    clusters
        .iter()
        .find(|c| c.name == cluster_name)
        .and_then(|c| c.hosts.first())
        .and_then(|h| site_from_fqdn(&h.network_address).or_else(|| site_from_fqdn(&h.name)))
}

pub(super) fn build_label_meta_level1(
    level_1: &str,
    aggregate_by_level_1: AggregateByLevel1Enum,
    clusters: &[Cluster],
) -> Option<LabelMeta> {
    match aggregate_by_level_1 {
        AggregateByLevel1Enum::Cluster => {
            let _ = clusters;
            None
        }
        AggregateByLevel1Enum::Host => Some(LabelMeta {
            host: Some(level_1.to_string()),
        }),
        AggregateByLevel1Enum::Owner => None,
    }
}

pub(super) fn build_label_meta_level2(
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

use crate::models::data_structure::cluster::Cluster;

pub(super) fn short_host_label(host: &str) -> String {
    host.split('.').next().unwrap_or(host).trim().to_string()
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

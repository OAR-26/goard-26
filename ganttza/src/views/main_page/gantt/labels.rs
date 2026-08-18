use std::collections::HashMap;
use crate::models::data_structure::strata::Strata;

pub(super) fn short_host_label(host: &str) -> String {
    host.split('.').next().unwrap_or(host).trim().to_string()
}

fn site_from_fqdn(host: &str) -> Option<String> {
    let mut parts = host.split('.');
    let _hostname = parts.next();
    let site = parts.next();
    site.filter(|s| !s.is_empty()).map(|s| s.to_string())
}

pub(super) fn site_for_cluster_name(
    cluster_name: &str,
    cluster_hosts: &HashMap<String, Vec<String>>,
    strata_by_host: &HashMap<String, Strata>,
) -> Option<String> {
    let hosts = cluster_hosts.get(cluster_name)?;
    let first_host = hosts.first()?;
    let strata = strata_by_host.get(first_host)?;
    let addr = strata.network_address.as_deref().unwrap_or("");
    site_from_fqdn(addr).or_else(|| site_from_fqdn(first_host))
}

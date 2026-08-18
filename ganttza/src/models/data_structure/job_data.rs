use std::collections::HashMap;

use super::{job::Job, marker::GanttMarker, resource::DeadInterval, strata::Strata};

/// Generic job/cluster/resource data store. No knowledge of where the data
/// came from (live polling, file import, anything else) — the binary writes
/// into these fields however it sees fit.
pub struct JobData {
    pub all_jobs: Vec<Job>,
    pub filtered_jobs: Vec<Job>,

    /// cluster name → sorted list of resource IDs in that cluster
    pub cluster_resource_ids: HashMap<String, Vec<u32>>,
    /// host name → sorted list of resource IDs on that host
    pub host_resource_ids: HashMap<String, Vec<u32>>,
    /// cluster name → list of host names in that cluster
    pub cluster_hosts: HashMap<String, Vec<String>>,

    pub strata_by_host: HashMap<String, Strata>,
    pub strata_by_resource_id: HashMap<u32, Strata>,
    pub dead_intervals: HashMap<u32, Vec<DeadInterval>>,
    /// resource_id → available_upto for Absent resources with a known return date.
    pub standby_upto: HashMap<u32, i64>,

    /// Active markers from the current data source.
    pub markers: Vec<GanttMarker>,

    /// XY series for the secondary plot panel: (series name, points).
    /// Empty when no external data is loaded — the Gantt may fall back to
    /// a computed estimate from job allocations.
    pub plot_series: Vec<(String, Vec<(i64, f64)>)>,
}

impl Default for JobData {
    fn default() -> Self {
        Self {
            all_jobs: Vec::new(),
            filtered_jobs: Vec::new(),
            cluster_resource_ids: HashMap::new(),
            host_resource_ids: HashMap::new(),
            cluster_hosts: HashMap::new(),
            strata_by_host: HashMap::new(),
            strata_by_resource_id: HashMap::new(),
            dead_intervals: HashMap::new(),
            standby_upto: HashMap::new(),
            markers: Vec::new(),
            plot_series: Vec::new(),
        }
    }
}

impl JobData {
    /// Rebuild the cluster/host index maps from strata_by_resource_id.
    /// Call this after strata_by_resource_id is populated.
    pub fn rebuild_cluster_index(&mut self) {
        self.cluster_resource_ids.clear();
        self.host_resource_ids.clear();
        self.cluster_hosts.clear();
        for (rid, strata) in &self.strata_by_resource_id {
            if let Some(cluster) = &strata.cluster {
                self.cluster_resource_ids.entry(cluster.clone()).or_default().push(*rid);
                if let Some(host) = &strata.host {
                    let hosts = self.cluster_hosts.entry(cluster.clone()).or_default();
                    if !hosts.contains(host) {
                        hosts.push(host.clone());
                    }
                }
            }
            if let Some(host) = &strata.host {
                self.host_resource_ids.entry(host.clone()).or_default().push(*rid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::data_structure::strata::Strata;

    fn strata(resource_id: u32, cluster: &str, host: &str) -> (u32, Strata) {
        (resource_id, Strata {
            resource_id: Some(resource_id),
            cluster: Some(cluster.to_string()),
            host: Some(host.to_string()),
            ..Strata::default()
        })
    }

    #[test]
    fn rebuild_populates_cluster_resource_ids() {
        let mut d = JobData::default();
        d.strata_by_resource_id.extend([strata(1, "cluster-a", "host1"), strata(2, "cluster-a", "host2")]);
        d.rebuild_cluster_index();
        let mut ids = d.cluster_resource_ids["cluster-a"].clone();
        ids.sort();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn rebuild_populates_host_resource_ids() {
        let mut d = JobData::default();
        d.strata_by_resource_id.extend([strata(1, "c", "host1"), strata(2, "c", "host1"), strata(3, "c", "host2")]);
        d.rebuild_cluster_index();
        let mut h1 = d.host_resource_ids["host1"].clone();
        h1.sort();
        assert_eq!(h1, vec![1, 2]);
        assert_eq!(d.host_resource_ids["host2"], vec![3]);
    }

    #[test]
    fn rebuild_cluster_hosts_no_duplicates() {
        let mut d = JobData::default();
        // two resources on same host in same cluster
        d.strata_by_resource_id.extend([strata(1, "c", "host1"), strata(2, "c", "host1")]);
        d.rebuild_cluster_index();
        assert_eq!(d.cluster_hosts["c"], vec!["host1"]);
    }

    #[test]
    fn rebuild_multiple_clusters() {
        let mut d = JobData::default();
        d.strata_by_resource_id.extend([
            strata(1, "cluster-a", "ha1"),
            strata(2, "cluster-b", "hb1"),
        ]);
        d.rebuild_cluster_index();
        assert!(d.cluster_resource_ids.contains_key("cluster-a"));
        assert!(d.cluster_resource_ids.contains_key("cluster-b"));
        assert!(!d.cluster_resource_ids["cluster-a"].contains(&2));
    }

    #[test]
    fn rebuild_clears_previous_state() {
        let mut d = JobData::default();
        d.strata_by_resource_id.extend([strata(1, "old-cluster", "h1")]);
        d.rebuild_cluster_index();
        assert!(d.cluster_resource_ids.contains_key("old-cluster"));

        d.strata_by_resource_id.clear();
        d.strata_by_resource_id.extend([strata(2, "new-cluster", "h2")]);
        d.rebuild_cluster_index();
        assert!(!d.cluster_resource_ids.contains_key("old-cluster"));
        assert!(d.cluster_resource_ids.contains_key("new-cluster"));
    }
}

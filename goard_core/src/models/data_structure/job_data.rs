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

    /// Raw energy series for the current data source: (series name, points).
    /// Empty when there's no measured energy data — the Gantt falls back to
    /// estimating from job allocations.
    pub energy_series: Vec<(String, Vec<(i64, f64)>)>,
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
            energy_series: Vec::new(),
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

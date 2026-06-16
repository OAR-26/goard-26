use std::collections::HashMap;

use super::{cluster::Cluster, job::Job, marker::GanttMarker, resource::DeadInterval, strata::Strata};

/// Generic job/cluster/resource data store. No knowledge of where the data
/// came from (live polling, file import, anything else) — the binary writes
/// into these fields however it sees fit.
pub struct JobData {
    pub all_jobs: Vec<Job>,
    pub filtered_jobs: Vec<Job>,

    pub all_clusters: Vec<Cluster>,

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
            all_clusters: Vec::new(),
            strata_by_host: HashMap::new(),
            strata_by_resource_id: HashMap::new(),
            dead_intervals: HashMap::new(),
            standby_upto: HashMap::new(),
            markers: Vec::new(),
            energy_series: Vec::new(),
        }
    }
}

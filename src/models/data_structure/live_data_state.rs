use std::collections::HashMap;

use super::{cluster::Cluster, job::Job, resource::DeadInterval, strata::Strata};

pub struct LiveDataState {
    pub all_jobs: Vec<Job>,
    /// Swap buffer — background thread writes here; promoted to all_jobs on resource tick.
    pub swap_all_jobs: Vec<Job>,
    pub filtered_jobs: Vec<Job>,

    pub all_clusters: Vec<Cluster>,
    pub swap_all_clusters: Vec<Cluster>,

    pub strata_by_host: HashMap<String, Strata>,
    pub strata_by_resource_id: HashMap<u32, Strata>,
    pub dead_intervals: HashMap<u32, Vec<DeadInterval>>,
    /// resource_id → available_upto for Absent resources with a known return date.
    pub standby_upto: HashMap<u32, i64>,

    /// Live-only caches — updated every resource tick regardless of active tab.
    /// Restored into strata_by_* when switching back to live.
    pub strata_by_resource_id_live: HashMap<u32, Strata>,
    pub strata_by_host_live: HashMap<String, Strata>,
}

impl Default for LiveDataState {
    fn default() -> Self {
        Self {
            all_jobs: Vec::new(),
            swap_all_jobs: Vec::new(),
            filtered_jobs: Vec::new(),
            all_clusters: Vec::new(),
            swap_all_clusters: Vec::new(),
            strata_by_host: HashMap::new(),
            strata_by_resource_id: HashMap::new(),
            dead_intervals: HashMap::new(),
            standby_upto: HashMap::new(),
            strata_by_resource_id_live: HashMap::new(),
            strata_by_host_live: HashMap::new(),
        }
    }
}

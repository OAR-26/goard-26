use super::cluster::Cluster;
use super::filters::JobFilters;
use super::job::Job;
use super::resource::Resource;
use super::strata::Strata;
use crate::models::data_structure::cpu::Cpu;
use crate::models::data_structure::host::Host;
use crate::models::data_structure::gantt_config::GanttConfig;
use crate::models::data_structure::resource::{DeadInterval, ResourceState};
use crate::models::utils::parser::get_dead_intervals_from_json;
use crate::models::utils::utils::{get_clusters_for_job, get_hosts_for_job};
use crate::views::components::dashboard_components::job_table_sorting::JobSortable;
use crate::views::view::ViewType;
use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/*
`ApplicationContext` is the central state container for the entire application.
It manages jobs, clusters, resources, and application state, including filtering mechanisms
and communication channels for data updates.
*/
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClusterPreset {
    pub name: String,
    pub clusters: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ImportedDataSource {
    pub name: String,
    pub file_path: Option<String>,
    pub file_type_name: String,
    pub visualization_targets: Vec<crate::models::file_types::VisualizationTarget>,
    pub jobs: Vec<Job>,
    pub clusters: Vec<Cluster>,
    pub strata: Vec<Strata>,
    /// Pre-computed energy series from file type that provides raw (timestamp_s, watts) data.
    pub raw_energy_series: Option<Vec<(i64, f64)>>,
}

pub struct ApplicationContext {
    pub all_jobs: Vec<Job>,
    pub swap_all_jobs: Vec<Job>, // Used to store all jobs when refreshing (and swapped with all_jobs when refreshing is done)
    pub filtered_jobs: Vec<Job>, // Subset of all_jobs that match the filters

    pub all_clusters: Vec<Cluster>,
    pub swap_all_clusters: Vec<Cluster>, // Used to store all clusters when refreshing (and swapped with all_clusters when refreshing is done)
    pub cluster_presets: Vec<ClusterPreset>, // saved cluster presets (admin only)

    // Application view state
    pub start_date: Arc<Mutex<DateTime<Local>>>,
    pub end_date: Arc<Mutex<DateTime<Local>>>,
    pub view_type: ViewType,
    pub is_loading: bool,
    pub user_connected: Option<String>,
    pub is_refreshing: Arc<Mutex<bool>>,
    pub refresh_rate: Arc<Mutex<u64>>,
    pub filters: JobFilters,

    // Communication channels for background data updates
    pub jobs_receiver: Receiver<Vec<Job>>,
    pub jobs_sender: Sender<Vec<Job>>,
    pub resources_receiver: Receiver<Vec<Strata>>,
    pub resources_sender: Sender<Vec<Strata>>,
    pub dead_intervals_receiver: Receiver<HashMap<u32, Vec<DeadInterval>>>,
    pub dead_intervals_sender: Sender<HashMap<u32, Vec<DeadInterval>>>,

    // Resource metadata indexed by host name (tooltip lookups for compute nodes).
    pub strata_by_host: HashMap<String, Strata>,
    // Resource metadata indexed by OAR resource_id — covers all resource types
    // (default/compute, kavlan, subnet, disk, etc.) so Gantt grouping works for any field.
    pub strata_by_resource_id: HashMap<u32, Strata>,
    // Dead/Absent/Suspected intervals per resource ID, from dead_resources in data.json.
    pub dead_intervals: HashMap<u32, Vec<DeadInterval>>,
    // Maps resource_id → available_upto timestamp for Absent resources that have a future availability date.
    pub standby_upto: HashMap<u32, i64>,
    // Loaded from config.json at startup.
    pub gantt_config: GanttConfig,

    pub font_size: i32,
    pub see_all_jobs: bool,

    // Current gantt view — updated each frame by GanttChart so the toolbar can show
    // view-specific stats instead of always showing clusters/hosts.
    pub current_gantt_view_name: String,
    pub current_gantt_view_levels: Vec<String>,
    pub current_gantt_view_summary_fields: Vec<String>,

    // UI requests (set by views, consumed by Menu/Options)
    pub theme_toggle_requested: bool,
    
    // File import functionality with tabbed interface
    pub imported_data_sources: Vec<ImportedDataSource>,
    pub current_data_source_index: usize, // 0 = live data, 1+ = imported files
    pub request_file_import: bool,

    // Live-only strata caches — updated every resource tick regardless of which tab is active.
    // Restored into strata_by_* when switching back to live so the display is instant.
    pub strata_by_resource_id_live: HashMap<u32, Strata>,
    pub strata_by_host_live: HashMap<String, Strata>,
}

impl ApplicationContext {
    pub fn check_job_update(&mut self) {
        if let Ok(new_jobs) = self.jobs_receiver.try_recv() {
            // Always keep swap buffer current so switching back to live is instant.
            self.swap_all_jobs = new_jobs;
            if self.current_data_source_index == 0 {
                self.is_loading = false;
            }
        }
    }

    /*
    Checks for and processes any new resource data received from the background thread.
     This method builds the hierarchical structure of clusters, hosts, CPUs, and resources
     from the flat resource data received.
     */
    pub fn check_ressource_update(&mut self) {
        if let Ok(new_resources) = self.resources_receiver.try_recv() {
            // Always keep live-only caches current so switching back to live is instant.
            self.strata_by_resource_id_live.clear();
            for r in new_resources.iter() {
                if let Some(rid) = r.resource_id {
                    self.strata_by_resource_id_live.insert(rid, r.clone());
                }
            }
            self.strata_by_host_live.clear();
            for r in new_resources.iter() {
                let host = r.host.as_deref().unwrap_or("").trim().to_string();
                let net = r.network_address.as_deref().unwrap_or("").trim().to_string();
                if !host.is_empty() {
                    self.strata_by_host_live.entry(host.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = host.split('.').next() {
                        if !short.is_empty() {
                            self.strata_by_host_live.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
                if !net.is_empty() {
                    self.strata_by_host_live.entry(net.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = net.split('.').next() {
                        if !short.is_empty() {
                            self.strata_by_host_live.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
            }

            // Don't overwrite imported-file view with live data.
            if self.current_data_source_index != 0 {
                return;
            }
            fn extract_ints_from_value(v: &Value) -> Vec<i32> {
                fn extract_ints_from_str(s: &str) -> Vec<i32> {
                    let mut out: Vec<i32> = Vec::new();
                    let mut cur: i64 = 0;
                    let mut in_num = false;
                    for ch in s.chars() {
                        if ch.is_ascii_digit() {
                            in_num = true;
                            cur = cur * 10 + (ch as i64 - '0' as i64);
                        } else if in_num {
                            if (0..=i32::MAX as i64).contains(&cur) {
                                out.push(cur as i32);
                            }
                            cur = 0;
                            in_num = false;
                        }
                    }
                    if in_num && (0..=i32::MAX as i64).contains(&cur) {
                        out.push(cur as i32);
                    }
                    out
                }

                match v {
                    Value::Null => Vec::new(),
                    Value::Bool(_) => Vec::new(),
                    Value::Number(n) => n
                        .as_i64()
                        .filter(|i| (0..=i32::MAX as i64).contains(i))
                        .map(|i| vec![i as i32])
                        .unwrap_or_default(),
                    Value::String(s) => extract_ints_from_str(s),
                    Value::Array(arr) => {
                        let mut all: Vec<i32> = Vec::new();
                        for x in arr {
                            all.extend(extract_ints_from_value(x));
                        }
                        all
                    }
                    Value::Object(_) => Vec::new(),
                }
            }

            // Build cpuset index list per host by aggregating resource-level cpuset values.
            // OAR resources often provide a scalar cpuset per resource; Grid5000 displays the
            // aggregated list at host level.
            let mut cpuset_by_host: HashMap<String, Vec<i32>> = HashMap::new();
            for r in new_resources.iter() {
                let host = r.host.as_deref().unwrap_or("").trim();
                if host.is_empty() {
                    continue;
                }
                if let Some(v) = r.cpuset.as_ref() {
                    let ints = extract_ints_from_value(v);
                    if !ints.is_empty() {
                        cpuset_by_host
                            .entry(host.to_string())
                            .or_default()
                            .extend(ints);
                    }
                }
            }

            // Index every resource by its OAR resource_id for generic field lookup.
            self.strata_by_resource_id.clear();
            for r in new_resources.iter() {
                if let Some(rid) = r.resource_id {
                    self.strata_by_resource_id.insert(rid, r.clone());
                }
            }

            // Build standby map: Absent resources with a future available_upto.
            let now = chrono::Utc::now().timestamp();
            self.standby_upto.clear();
            for r in new_resources.iter() {
                if r.state.as_deref() == Some("Absent") {
                    if let (Some(rid), Some(upto)) = (r.resource_id, r.available_upto) {
                        if upto > now && upto > 0 {
                            self.standby_upto.insert(rid, upto);
                        }
                    }
                }
            }

            // Cache the latest metadata for tooltips. Use multiple keys per host to be robust
            // (short host, FQDN, network_address).
            self.strata_by_host.clear();
            for r in new_resources.iter() {
                let host = r.host.as_deref().unwrap_or("").trim();
                let net = r.network_address.as_deref().unwrap_or("").trim();

                if !host.is_empty() {
                    self.strata_by_host
                        .entry(host.to_string())
                        .or_insert_with(|| r.clone());
                    let short = host.split('.').next().unwrap_or(host).trim();
                    if !short.is_empty() {
                        self.strata_by_host
                            .entry(short.to_string())
                            .or_insert_with(|| r.clone());
                    }
                }

                if !net.is_empty() {
                    self.strata_by_host
                        .entry(net.to_string())
                        .or_insert_with(|| r.clone());
                    let short = net.split('.').next().unwrap_or(net).trim();
                    if !short.is_empty() {
                        self.strata_by_host
                            .entry(short.to_string())
                            .or_insert_with(|| r.clone());
                    }
                }

                // Prefer a record that has more human-friendly fields filled.
                // This updates an already-inserted entry if the new record is "better".
                fn non_empty_value(v: &Value) -> bool {
                    match v {
                        Value::Null => false,
                        Value::Bool(_) => true,
                        Value::Number(_) => true,
                        Value::String(s) => !s.trim().is_empty(),
                        Value::Array(arr) => arr.iter().any(non_empty_value),
                        Value::Object(obj) => !obj.is_empty(),
                    }
                }
                for k in [host, net] {
                    if k.is_empty() {
                        continue;
                    }
                    if let Some(existing) = self.strata_by_host.get(k).cloned() {
                        let existing_score = existing
                            .comment
                            .as_ref()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false) as i32
                            + existing
                                .cpuset
                                .as_ref()
                                .map(non_empty_value)
                                .unwrap_or(false) as i32
                            + existing
                                .cputype
                                .as_ref()
                                .map(|s| !s.trim().is_empty())
                                .unwrap_or(false) as i32
                            + existing
                                .nodemodel
                                .as_ref()
                                .map(|s| !s.trim().is_empty())
                                .unwrap_or(false) as i32;
                        let new_score = r
                            .comment
                            .as_ref()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false) as i32
                            + r
                                .cpuset
                                .as_ref()
                                .map(non_empty_value)
                                .unwrap_or(false) as i32
                            + r
                                .cputype
                                .as_ref()
                                .map(|s| !s.trim().is_empty())
                                .unwrap_or(false) as i32
                            + r
                                .nodemodel
                                .as_ref()
                                .map(|s| !s.trim().is_empty())
                                .unwrap_or(false) as i32;
                        if new_score > existing_score {
                            self.strata_by_host.insert(k.to_string(), r.clone());
                        }
                    }
                }
            }

            // Overwrite cached cpuset with the aggregated host-level cpuset list (when available).
            for s in self.strata_by_host.values_mut() {
                let host_key = s.host.as_deref().unwrap_or("").trim();
                if host_key.is_empty() {
                    continue;
                }
                if let Some(ints) = cpuset_by_host.get(host_key) {
                    let mut ints = ints.clone();
                    ints.sort_unstable();
                    ints.dedup();
                    if !ints.is_empty() {
                        let arr: Vec<Value> = ints
                            .into_iter()
                            .map(|i| Value::Number(serde_json::Number::from(i)))
                            .collect();
                        s.cpuset = Some(Value::Array(arr));
                    }
                }
            }

            // for every resources get the cluster name with resource.cluster and if there is no cluster with this name in all_clusters add it to all_clusters
            for resource in new_resources.iter() {
                let cluster_name = resource.cluster.as_ref().unwrap_or(&"".to_string()).clone();
                if cluster_name == "" {
                    continue;
                }
                if !self
                    .swap_all_clusters
                    .iter()
                    .any(|cluster| cluster.name == cluster_name)
                {
                    // Add the cluster to all_clusters with one host being resource.host
                    let new_cluster = Cluster {
                        name: cluster_name.clone(),
                        hosts: vec![Host {
                            name: resource.host.as_ref().unwrap_or(&"".to_string()).clone(),
                            cpus: vec![Cpu {
                                name: resource.cputype.as_ref().unwrap_or(&"".to_string()).clone(),
                                resources: vec![Resource {
                                    id: resource.resource_id.unwrap_or(0),
                                    state: match resource
                                        .state
                                        .as_ref()
                                        .unwrap_or(&"".to_string())
                                        .as_str()
                                    {
                                        "Dead" => super::resource::ResourceState::Dead,
                                        "Alive" => super::resource::ResourceState::Alive,
                                        "Absent" => super::resource::ResourceState::Absent,
                                        _ => super::resource::ResourceState::Unknown,
                                    },
                                    thread_count: resource.thread_count.unwrap_or(0) as i32,
                                }],
                                core_count: resource.core_count.unwrap_or(0) as i32,
                                cpufreq: resource
                                    .cpufreq
                                    .as_ref()
                                    .unwrap_or(&"".to_string())
                                    .parse::<f32>()
                                    .unwrap_or(0.0),
                                chassis: resource
                                    .chassis
                                    .as_ref()
                                    .unwrap_or(&"".to_string())
                                    .clone(),
                                resource_ids: vec![resource.resource_id.unwrap_or(0)],
                            }],
                            network_address: resource
                                .network_address
                                .as_ref()
                                .unwrap_or(&"".to_string())
                                .clone(),
                            resource_ids: vec![resource.resource_id.unwrap_or(0)],
                            state: ResourceState::Unknown,
                        }],
                        resource_ids: vec![resource.resource_id.unwrap_or(0)],
                        state: ResourceState::Unknown,
                    };

                    // Add the cluster to all_clusters
                    self.swap_all_clusters.push(new_cluster);
                } else {
                    // if the cluster already exists, check if the host exists and add the host if it doesn't
                    let cluster = self
                        .swap_all_clusters
                        .iter_mut()
                        .find(|cluster| cluster.name == cluster_name)
                        .unwrap();
                    if !cluster.hosts.iter().any(|host| {
                        host.name == resource.host.as_ref().unwrap_or(&"".to_string()).clone()
                    }) {
                        cluster.hosts.push(Host {
                            name: resource.host.as_ref().unwrap_or(&"".to_string()).clone(),
                            cpus: vec![Cpu {
                                name: resource.cputype.as_ref().unwrap_or(&"".to_string()).clone(),
                                resources: vec![Resource {
                                    id: resource.resource_id.unwrap_or(0),
                                    state: match resource
                                        .state
                                        .as_ref()
                                        .unwrap_or(&"".to_string())
                                        .as_str()
                                    {
                                        "Dead" => super::resource::ResourceState::Dead,
                                        "Alive" => super::resource::ResourceState::Alive,
                                        "Absent" => super::resource::ResourceState::Absent,
                                        _ => super::resource::ResourceState::Unknown,
                                    },
                                    thread_count: resource.thread_count.unwrap_or(0) as i32,
                                }],
                                core_count: resource.core_count.unwrap_or(0) as i32,
                                cpufreq: resource
                                    .cpufreq
                                    .as_ref()
                                    .unwrap_or(&"".to_string())
                                    .parse::<f32>()
                                    .unwrap_or(0.0),
                                chassis: resource
                                    .chassis
                                    .as_ref()
                                    .unwrap_or(&"".to_string())
                                    .clone(),
                                resource_ids: vec![resource.resource_id.unwrap_or(0)],
                            }],
                            network_address: resource
                                .network_address
                                .as_ref()
                                .unwrap_or(&"".to_string())
                                .clone(),
                            resource_ids: vec![resource.resource_id.unwrap_or(0)],
                            state: ResourceState::Unknown,
                        });
                        // add the resource id to the cluster
                        cluster.resource_ids.push(resource.resource_id.unwrap_or(0));
                    } else {
                        // if the host already exists, check if the cpu exists and add the cpu if it doesn't
                        let host = cluster
                            .hosts
                            .iter_mut()
                            .find(|host| {
                                host.name
                                    == resource.host.as_ref().unwrap_or(&"".to_string()).clone()
                            })
                            .unwrap();
                        if !host.cpus.iter().any(|cpu| {
                            cpu.name == resource.cputype.as_ref().unwrap_or(&"".to_string()).clone()
                        }) {
                            host.cpus.push(Cpu {
                                name: resource.cputype.as_ref().unwrap_or(&"".to_string()).clone(),
                                resources: vec![Resource {
                                    id: resource.resource_id.unwrap_or(0),
                                    state: match resource
                                        .state
                                        .as_ref()
                                        .unwrap_or(&"".to_string())
                                        .as_str()
                                    {
                                        "Dead" => super::resource::ResourceState::Dead,
                                        "Alive" => super::resource::ResourceState::Alive,
                                        "Absent" => super::resource::ResourceState::Absent,
                                        _ => super::resource::ResourceState::Unknown,
                                    },
                                    thread_count: resource.thread_count.unwrap_or(0) as i32,
                                }],
                                core_count: resource.core_count.unwrap_or(0) as i32,
                                cpufreq: resource
                                    .cpufreq
                                    .as_ref()
                                    .unwrap_or(&"".to_string())
                                    .parse::<f32>()
                                    .unwrap_or(0.0),
                                chassis: resource
                                    .chassis
                                    .as_ref()
                                    .unwrap_or(&"".to_string())
                                    .clone(),
                                resource_ids: vec![resource.resource_id.unwrap_or(0)],
                            });

                            // add the resource id to the host and the cluster
                            host.resource_ids.push(resource.resource_id.unwrap_or(0));
                            cluster.resource_ids.push(resource.resource_id.unwrap_or(0));
                        } else {
                            // if the cpu already exists, add the resource to the cpu
                            let cpu = host
                                .cpus
                                .iter_mut()
                                .find(|cpu| {
                                    cpu.name
                                        == resource
                                            .cputype
                                            .as_ref()
                                            .unwrap_or(&"".to_string())
                                            .clone()
                                })
                                .unwrap();
                            cpu.resources.push(Resource {
                                id: resource.resource_id.unwrap_or(0),
                                state: match resource
                                    .state
                                    .as_ref()
                                    .unwrap_or(&"".to_string())
                                    .as_str()
                                {
                                    "Dead" => super::resource::ResourceState::Dead,
                                    "Alive" => super::resource::ResourceState::Alive,
                                    "Absent" => super::resource::ResourceState::Absent,
                                    _ => super::resource::ResourceState::Unknown,
                                },
                                thread_count: resource.thread_count.unwrap_or(0) as i32,
                            });

                            // add the resource id to the cpu, the host and the cluster
                            cpu.resource_ids.push(resource.resource_id.unwrap_or(0));
                            host.resource_ids.push(resource.resource_id.unwrap_or(0));
                            cluster.resource_ids.push(resource.resource_id.unwrap_or(0));
                        }
                    }
                }
            }
            for job in self.swap_all_jobs.iter_mut() {
                job.clusters = get_clusters_for_job(job, &self.swap_all_clusters);
                job.hosts = get_hosts_for_job(job, &self.swap_all_clusters);
                job.update_majority_resource_state(&self.swap_all_clusters);
            }

            // For each host set is state to the state the most resources have
            for cluster in self.swap_all_clusters.iter_mut() {
                for host in cluster.hosts.iter_mut() {
                    let mut dead_count = 0;
                    let mut alive_count = 0;
                    let mut absent_count = 0;
                    for cpu in host.cpus.iter() {
                        for resource in cpu.resources.iter() {
                            match resource.state {
                                ResourceState::Dead => dead_count += 1,
                                ResourceState::Alive => alive_count += 1,
                                ResourceState::Absent => absent_count += 1,
                                _ => (),
                            }
                        }
                    }
                    if dead_count >= alive_count && dead_count >= absent_count {
                        host.state = ResourceState::Dead;
                    } else if absent_count >= dead_count && absent_count >= alive_count {
                        host.state = ResourceState::Absent;
                    } else if alive_count > dead_count && alive_count > absent_count {
                        host.state = ResourceState::Alive;
                    } else {
                        host.state = ResourceState::Unknown;
                    }
                }
            }

            // For each cluster set is state to the state the most hosts have
            for cluster in self.swap_all_clusters.iter_mut() {
                let mut dead_count = 0;
                let mut alive_count = 0;
                let mut absent_count = 0;
                for host in cluster.hosts.iter() {
                    match host.state {
                        ResourceState::Dead => dead_count += 1,
                        ResourceState::Alive => alive_count += 1,
                        ResourceState::Absent => absent_count += 1,
                        _ => (),
                    }
                }
                if dead_count >= alive_count && dead_count >= absent_count {
                    cluster.state = ResourceState::Dead;
                } else if absent_count >= dead_count && absent_count >= alive_count {
                    cluster.state = ResourceState::Absent;
                } else if alive_count > dead_count && alive_count > absent_count {
                    cluster.state = ResourceState::Alive;
                } else {
                    cluster.state = ResourceState::Unknown;
                }
            }
            // Swap all_jobs and all_clusters with swap_all_jobs and swap_all_clusters
            // If there is a job with id 0 in all_jobs, we keep it
            let has_job_0 = self.all_jobs.iter().any(|job| job.id == 0);
            if has_job_0 {
                // Get the job with id 0
                let job_0 = self
                    .all_jobs
                    .iter()
                    .find(|job| job.id == 0)
                    .unwrap()
                    .clone();
                self.swap_all_jobs.push(job_0);
            }

            self.all_jobs = self.swap_all_jobs.clone();
            self.all_clusters = self.swap_all_clusters.clone();
        }
    }

    pub fn check_dead_intervals_update(&mut self) {
        if self.current_data_source_index != 0 {
            return;
        }
        if let Ok(intervals) = self.dead_intervals_receiver.try_recv() {
            self.dead_intervals = intervals;
        }
    }

    pub fn check_data_update(&mut self) {
        self.check_job_update();
        self.check_ressource_update();
        self.check_dead_intervals_update();

        // set filter date to the date of the app context
        self.filters
            .set_scheduled_start_time(self.start_date.lock().unwrap().timestamp());
        self.filters
            .set_wall_time(self.end_date.lock().unwrap().timestamp());

        self.filter_jobs();
    }

    pub fn logout(&mut self) {
        self.user_connected = None;
        self.view_type = ViewType::Authentification;
    }

    /// Simple helper to determine if the currently connected user is the hard-coded admin.
    pub fn is_admin(&self) -> bool {
        matches!(self.user_connected.as_deref(), Some("admin"))
    }

    /// Persist presets list to `presets.json` in the working directory.
    fn save_presets_to_file(&self, file_path: &str) {
        if let Ok(json) = serde_json::to_string(&self.cluster_presets) {
            // ignore write errors; could log if needed
            let _ = std::fs::write(file_path, json);
        }
    }

    /// Load presets from the given file, returning an empty vec on error.
    fn load_presets_from_file(file_path: &str) -> Vec<ClusterPreset> {
        match std::fs::read_to_string(file_path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    /// Add a new cluster preset or update an existing one with the same name.
    /// If the operation succeeds we also write the updated list to disk.
    pub fn add_or_update_preset(&mut self, preset: ClusterPreset) {
        if let Some(existing) = self
            .cluster_presets
            .iter_mut()
            .find(|p| p.name == preset.name)
        {
            *existing = preset;
        } else {
            self.cluster_presets.push(preset);
        }
        // persist immediately
        self.save_presets_to_file("presets.json");
    }

    /// Remove a cluster preset by name.
    /// If the operation succeeds we also write the updated list to disk.
    pub fn remove_preset(&mut self, name: &str) {
        self.cluster_presets.retain(|p| p.name != name);
        // persist immediately
        self.save_presets_to_file("presets.json");
    }

    pub fn login(&mut self, username: &str) {
        self.user_connected = Some(username.to_string());
        self.view_type = ViewType::Dashboard;
    }

    /* Returns a deduplicated, sorted list of all unique job owners
     * Used for filtering functionality in the UI
     */
    pub fn get_unique_owners(&self) -> Vec<String> {
        let mut owners: Vec<String> = self.all_jobs.iter().map(|job| job.owner.clone()).collect();
        // remove the owner all_resources if it exists
        owners.retain(|owner| owner != "all_resources");
        owners.sort();
        owners.dedup();
        owners
    }

    /*
     * Applies the current filters to all_jobs and updates filtered_jobs
     * This method handles all filtering logic including:
     * - Job owner filtering
     * - Job state filtering
     * - Time range filtering
     * - Cluster resource filtering
     */
    pub fn filter_jobs(&mut self) {
        let current_jobs = self.get_current_jobs();
        
        // Determine the selected clusters from the preset, if any
        let selected_cluster_names: Option<Vec<String>> = self.filters.selected_preset.as_ref()
            .and_then(|preset_name| self.cluster_presets.iter().find(|p| p.name == *preset_name))
            .map(|preset| preset.clusters.clone());

        self.filtered_jobs = current_jobs
            .iter()
            .filter(|job| {
                job.id == 0
                    || (self
                        .filters
                        .owners
                        .as_ref()
                        .map_or(true, |owners| owners.contains(&job.owner)))
                        && (self
                            .filters
                            .states
                            .as_ref()
                            .map_or(true, |states| states.contains(&job.state)))
                        && (((self
                            .filters
                            .scheduled_start_time
                            .map_or(true, |time| time <= job.scheduled_start))
                            && (self
                                .filters
                                .wall_time
                                .map_or(true, |time| time >= job.scheduled_start)))
                            || ((self
                                .filters
                                .scheduled_start_time
                                .map_or(true, |time| time <= job.get_end_date()))
                                && (self
                                    .filters
                                    .wall_time
                                    .map_or(true, |time| time >= job.get_end_date())))
                            || ((self
                                .filters
                                .scheduled_start_time
                                .map_or(true, |time| time >= job.start_time))
                                && (self
                                    .filters
                                    .wall_time
                                    .map_or(true, |time| time <= job.get_end_date()))))
                        && (selected_cluster_names.is_none() || {
                            let cluster_names = selected_cluster_names.as_ref().unwrap();
                            cluster_names.iter().any(|cluster_name| job.clusters.contains(cluster_name))
                        })
            })
            .cloned() // Clone filtred jobs here
            .collect();
    }

    pub fn import_data_from_json(&mut self, json_str: &str, file_path: Option<String>) -> Result<(), String> {
        use crate::models::file_types::FileTypeRegistry;

        let registry = FileTypeRegistry::default();

        let file_type = registry
            .detect(json_str)
            .ok_or_else(|| "Unrecognized file format — no matching file type found.".to_string())?;

        let errors = file_type.validate(json_str);
        if !errors.is_empty() {
            let msg = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
            return Err(format!("Validation failed: {}", msg));
        }

        let parsed = file_type.parse(json_str)?;

        let base_name = file_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Data");
        let name = self.generate_unique_name(base_name);

        self.imported_data_sources.push(ImportedDataSource {
            name,
            file_path,
            file_type_name: file_type.name().to_string(),
            visualization_targets: file_type.visualization_targets().to_vec(),
            raw_energy_series: parsed.raw_energy_series,
            jobs: parsed.jobs,
            clusters: parsed.clusters,
            strata: parsed.resources,
        });

        let new_index = self.imported_data_sources.len();
        self.switch_to_data_source(new_index);

        Ok(())
    }

    fn generate_unique_name(&self, base_name: &str) -> String {
        let mut name = base_name.to_string();
        let mut counter = 1;
        
        while self.imported_data_sources.iter().any(|ds| ds.name == name) {
            name = format!("{} ({})", base_name, counter);
            counter += 1;
        }
        
        name
    }
    
    pub fn get_current_data_source_name(&self) -> String {
        if self.current_data_source_index == 0 {
            "Live Data".to_string()
        } else {
            self.imported_data_sources
                .get(self.current_data_source_index - 1)
                .map(|ds| ds.name.clone())
                .unwrap_or("Unknown".to_string())
        }
    }
    
    pub fn switch_to_data_source(&mut self, index: usize) {
        if index == 0 {
            self.current_data_source_index = 0;
            // Immediately show last-known live state without waiting for next background tick.
            self.all_jobs = self.swap_all_jobs.clone();
            self.all_clusters = self.swap_all_clusters.clone();
            if !self.strata_by_resource_id_live.is_empty() {
                self.strata_by_resource_id = self.strata_by_resource_id_live.clone();
                self.strata_by_host = self.strata_by_host_live.clone();
            }
            // Reload dead intervals immediately from disk (avoids waiting for next 30s tick).
            self.dead_intervals = get_dead_intervals_from_json("./data/data.json");
            // Reset time range to current live window so initial_start_s is correct.
            let live_now = Local::now();
            self.set_localdate(
                live_now - chrono::Duration::hours(1),
                live_now + chrono::Duration::hours(1),
            );
            // Rebuild standby_upto from the live strata cache.
            let now = chrono::Utc::now().timestamp();
            self.standby_upto.clear();
            for (rid, r) in &self.strata_by_resource_id_live {
                if r.state.as_deref() == Some("Absent") {
                    if let Some(upto) = r.available_upto {
                        if upto > now && upto > 0 {
                            self.standby_upto.insert(*rid, upto);
                        }
                    }
                }
            }
        } else if self.imported_data_sources.get(index - 1).is_some() {
            self.current_data_source_index = index;
            // Clear live-specific state so it doesn't bleed into the imported view.
            self.dead_intervals.clear();
            self.standby_upto.clear();
            // Rebuild strata from the imported source so resource states are correct.
            let strata = self.imported_data_sources[index - 1].strata.clone();
            self.strata_by_resource_id.clear();
            self.strata_by_host.clear();
            for r in &strata {
                if let Some(rid) = r.resource_id {
                    self.strata_by_resource_id.insert(rid, r.clone());
                }
                let host = r.host.as_deref().unwrap_or("").trim().to_string();
                let net = r.network_address.as_deref().unwrap_or("").trim().to_string();
                if !host.is_empty() {
                    self.strata_by_host.entry(host.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = host.split('.').next() {
                        if !short.is_empty() {
                            self.strata_by_host.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
                if !net.is_empty() {
                    self.strata_by_host.entry(net.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = net.split('.').next() {
                        if !short.is_empty() {
                            self.strata_by_host.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
            }
        } else {
            return;
        }
        // Get the jobs for the current data source and collect needed data
        let jobs: Vec<Job> = self.get_current_jobs().to_vec();
        let jobs_count = jobs.len();
        
        // Adjust time range for imported data.
        if index != 0 {
            let mut min_time = i64::MAX;
            let mut max_time = i64::MIN;

            // Range from jobs.
            for job in jobs.iter().filter(|j| j.id != 0) {
                min_time = min_time.min(job.start_time).min(job.get_end_date());
                max_time = max_time.max(job.start_time).max(job.get_end_date());
            }

            // Range from raw energy series when no jobs provide timestamps.
            if min_time == i64::MAX {
                if let Some(series) = self.imported_data_sources
                    .get(index - 1)
                    .and_then(|ds| ds.raw_energy_series.as_deref())
                {
                    for &(ts, _) in series {
                        min_time = min_time.min(ts);
                        max_time = max_time.max(ts);
                    }
                }
            }

            if min_time != i64::MAX && max_time != i64::MIN {
                let padding = (max_time - min_time) / 10;
                let start_time = Local.timestamp_opt(min_time - padding, 0).unwrap();
                let end_time = Local.timestamp_opt(max_time + padding, 0).unwrap();
                self.set_localdate(start_time, end_time);
            }
        }
        
        // Re-filter jobs
        self.filter_jobs();
    }
    
    pub fn close_imported_data_source(&mut self, index: usize) -> bool {
        if index == 0 {
            return false; // Cannot close live data
        }
        
        let actual_index = index - 1;
        if actual_index < self.imported_data_sources.len() {
            self.imported_data_sources.remove(actual_index);
            
            // Adjust current index if necessary
            if self.current_data_source_index > index {
                self.current_data_source_index -= 1;
            } else if self.current_data_source_index == index {
                // If we closed the current tab, switch to live data
                self.current_data_source_index = 0;
            }
            
            // Re-filter jobs
            self.filter_jobs();
            return true;
        }
        
        false
    }
    
    pub fn get_all_data_source_names(&self) -> Vec<String> {
        let mut names = vec!["Live Data".to_string()];
        for ds in &self.imported_data_sources {
            names.push(ds.name.clone());
        }
        names
    }
    
    pub fn get_current_jobs(&self) -> &[Job] {
        if self.current_data_source_index == 0 {
            &self.all_jobs
        } else {
            self.imported_data_sources
                .get(self.current_data_source_index - 1)
                .map(|ds| &ds.jobs)
                .unwrap_or(&self.all_jobs)
        }
    }
    
    pub fn get_current_clusters(&self) -> &Vec<Cluster> {
        if self.current_data_source_index == 0 {
            &self.all_clusters
        } else {
            self.imported_data_sources
                .get(self.current_data_source_index - 1)
                .map(|ds| &ds.clusters)
                .unwrap_or(&self.all_clusters)
        }
    }

    /// True when the currently active data source should show the energy diagram.
    /// Live data always shows both visualizations.
    pub fn show_energy_diagram(&self) -> bool {
        use crate::models::file_types::VisualizationTarget;
        if self.current_data_source_index == 0 {
            return true;
        }
        self.imported_data_sources
            .get(self.current_data_source_index - 1)
            .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::EnergyDiagram))
            .unwrap_or(true)
    }

    /// True when the currently active data source should show the Gantt chart.
    /// Live data always shows both visualizations.
    pub fn show_gantt(&self) -> bool {
        use crate::models::file_types::VisualizationTarget;
        if self.current_data_source_index == 0 {
            return true;
        }
        self.imported_data_sources
            .get(self.current_data_source_index - 1)
            .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::Gantt))
            .unwrap_or(true)
    }

    /// Returns energy series from the current imported source, if any.
    /// When Some, the energy diagram uses this directly instead of estimating from jobs.
    pub fn get_current_energy_series(&self) -> Option<&[(i64, f64)]> {
        if self.current_data_source_index == 0 {
            return None;
        }
        self.imported_data_sources
            .get(self.current_data_source_index - 1)
            .and_then(|ds| ds.raw_energy_series.as_deref())
    }
}

impl Default for ApplicationContext {
    // Creates a default ApplicationContext with initial values and sets up the background
    // data refresh mechanism.
    fn default() -> Self {
        let (jobs_sender, jobs_receiver) = channel();
        let (resources_sender, resources_receiver) = channel();
        let (dead_intervals_sender, dead_intervals_receiver) = channel();

        let now: DateTime<Local> = Local::now();
        let mut context = Self {
            all_jobs: Vec::new(),
            all_clusters: Vec::new(),

            swap_all_jobs: Vec::new(),
            swap_all_clusters: Vec::new(),

            jobs_receiver: jobs_receiver,
            jobs_sender: jobs_sender,
            resources_receiver: resources_receiver,
            resources_sender: resources_sender,
            dead_intervals_receiver,
            dead_intervals_sender,
            user_connected: None,

            strata_by_host: HashMap::new(),
            strata_by_resource_id: HashMap::new(),
            dead_intervals: HashMap::new(),
            standby_upto: HashMap::new(),
            gantt_config: GanttConfig::load(),

            filtered_jobs: Vec::new(),
            filters: JobFilters::default(),
            start_date: Arc::new(Mutex::new(now - chrono::Duration::hours(1))),
            end_date: Arc::new(Mutex::new(now + chrono::Duration::hours(1))),
            view_type: ViewType::Gantt,
            is_loading: false,
            is_refreshing: Arc::new(Mutex::new(false)),
            refresh_rate: Arc::new(Mutex::new(30)),

            font_size: 16,
            see_all_jobs: false,

            current_gantt_view_name: String::new(),
            current_gantt_view_levels: Vec::new(),
            current_gantt_view_summary_fields: Vec::new(),

            theme_toggle_requested: false,
            cluster_presets: Vec::new(),
            
            imported_data_sources: Vec::new(),
            current_data_source_index: 0, // Start with live data
            request_file_import: false,
            strata_by_resource_id_live: HashMap::new(),
            strata_by_host_live: HashMap::new(),
        };
        
        // populate presets from disk if available
        context.cluster_presets = ApplicationContext::load_presets_from_file("presets.json");
        context.update_periodically();
        context
    }
}

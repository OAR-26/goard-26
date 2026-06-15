use super::filters::JobFilters;
use super::job::Job;
use super::cluster::Cluster;
use super::resource::ResourceState;
use super::cpu::Cpu;
use super::host::Host;
use super::resource::Resource;
use super::live_data_state::LiveDataState;
use super::refresh_coordinator::RefreshCoordinator;
use super::ui_preferences::UiPreferences;
use crate::models::utils::utils::{get_clusters_for_job, get_hosts_for_job};
use crate::models::data_structure::job_sorting::JobSortable;
use crate::models::data_structure::view_type::ViewType;
use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::HashMap;

// Re-export so external callers can keep using `application_context::ClusterPreset`
pub use super::ui_preferences::ClusterPreset;

pub struct ApplicationContext {
    pub data: LiveDataState,
    pub refresh: RefreshCoordinator,
    pub prefs: UiPreferences,

    // Session state
    pub view_type: ViewType,
    pub is_loading: bool,
    pub filters: JobFilters,
    pub live_data: bool,

    // Direct time window — kept in sync with refresh.start_date/end_date.
    // Views read these directly instead of locking the Mutex every frame.
    pub start_date: DateTime<Local>,
    pub end_date: DateTime<Local>,

    // Runtime state synced from RefreshCoordinator each frame by check_data_update().
    pub is_refreshing: bool,
    pub live_refresh_paused: bool,

    // Signals set by views; consumed and acted on by the binary-level App (liveOAR).
    pub refresh_requested: bool,
    pub live_disable_requested: bool,

    // Flat display state — set by the binary when switching sources.
    // Core has no knowledge of how sources are managed; it just renders these fields.
    pub current_source_key: usize,
    pub current_source_name: String,
    pub current_group_active: bool,
    pub current_energy_series: Vec<(String, Vec<(i64, f64)>)>,
    pub show_energy: bool,
    pub show_gantt_panel: bool,
    pub current_file_path: Option<String>,
    pub current_file_hash: Option<String>,
    pub current_file_type_name: String,
    pub current_supports_hierarchy: bool,
}

impl ApplicationContext {
    pub fn check_job_update(&mut self) {
        if let Ok(new_jobs) = self.refresh.jobs_receiver.try_recv() {
            if !self.live_data { return; }
            self.data.swap_all_jobs = new_jobs;
            self.is_loading = false;
        }
    }

    pub fn check_ressource_update(&mut self) {
        if let Ok(new_resources) = self.refresh.resources_receiver.try_recv() {
            if !self.live_data { return; }
            // Always keep live-only caches current so switching back to live is instant.
            self.data.strata_by_resource_id_live.clear();
            for r in new_resources.iter() {
                if let Some(rid) = r.resource_id {
                    self.data.strata_by_resource_id_live.insert(rid, r.clone());
                }
            }
            self.data.strata_by_host_live.clear();
            for r in new_resources.iter() {
                let host = r.host.as_deref().unwrap_or("").trim().to_string();
                let net = r.network_address.as_deref().unwrap_or("").trim().to_string();
                if !host.is_empty() {
                    self.data.strata_by_host_live.entry(host.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = host.split('.').next() {
                        if !short.is_empty() {
                            self.data.strata_by_host_live.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
                if !net.is_empty() {
                    self.data.strata_by_host_live.entry(net.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = net.split('.').next() {
                        if !short.is_empty() {
                            self.data.strata_by_host_live.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
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

            self.data.strata_by_resource_id.clear();
            for r in new_resources.iter() {
                if let Some(rid) = r.resource_id {
                    self.data.strata_by_resource_id.insert(rid, r.clone());
                }
            }

            let now = chrono::Utc::now().timestamp();
            self.data.standby_upto.clear();
            for r in new_resources.iter() {
                if r.state.as_deref() == Some("Absent") {
                    if let (Some(rid), Some(upto)) = (r.resource_id, r.available_upto) {
                        if upto > now && upto > 0 {
                            self.data.standby_upto.insert(rid, upto);
                        }
                    }
                }
            }

            self.data.strata_by_host.clear();
            for r in new_resources.iter() {
                let host = r.host.as_deref().unwrap_or("").trim();
                let net = r.network_address.as_deref().unwrap_or("").trim();

                if !host.is_empty() {
                    self.data.strata_by_host
                        .entry(host.to_string())
                        .or_insert_with(|| r.clone());
                    let short = host.split('.').next().unwrap_or(host).trim();
                    if !short.is_empty() {
                        self.data.strata_by_host
                            .entry(short.to_string())
                            .or_insert_with(|| r.clone());
                    }
                }

                if !net.is_empty() {
                    self.data.strata_by_host
                        .entry(net.to_string())
                        .or_insert_with(|| r.clone());
                    let short = net.split('.').next().unwrap_or(net).trim();
                    if !short.is_empty() {
                        self.data.strata_by_host
                            .entry(short.to_string())
                            .or_insert_with(|| r.clone());
                    }
                }

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
                    if let Some(existing) = self.data.strata_by_host.get(k).cloned() {
                        let existing_score = existing.comment.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                            + existing.cpuset.as_ref().map(non_empty_value).unwrap_or(false) as i32
                            + existing.cputype.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                            + existing.nodemodel.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32;
                        let new_score = r.comment.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                            + r.cpuset.as_ref().map(non_empty_value).unwrap_or(false) as i32
                            + r.cputype.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                            + r.nodemodel.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32;
                        if new_score > existing_score {
                            self.data.strata_by_host.insert(k.to_string(), r.clone());
                        }
                    }
                }
            }

            for s in self.data.strata_by_host.values_mut() {
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

            // Build cluster hierarchy in O(n) using index maps for find-or-create.
            // Background thread sends a complete resource snapshot each cycle, so we
            // clear and rebuild from scratch to avoid stale/duplicate entries.
            self.data.swap_all_clusters.clear();
            let mut cluster_idx: HashMap<String, usize> = HashMap::new();
            let mut host_idx: HashMap<(usize, String), usize> = HashMap::new();
            let mut cpu_idx: HashMap<(usize, usize, String), usize> = HashMap::new();

            let parse_state = |s: Option<&str>| match s.unwrap_or("") {
                "Dead"   => ResourceState::Dead,
                "Alive"  => ResourceState::Alive,
                "Absent" => ResourceState::Absent,
                _        => ResourceState::Unknown,
            };

            for resource in new_resources.iter() {
                let cluster_name = resource.cluster.as_deref().unwrap_or("").trim().to_string();
                if cluster_name.is_empty() { continue; }
                let host_name = resource.host.as_deref().unwrap_or("").trim().to_string();
                let cpu_name  = resource.cputype.as_deref().unwrap_or("").trim().to_string();
                let rid       = resource.resource_id.unwrap_or(0);
                let state     = parse_state(resource.state.as_deref());

                // O(1) find-or-create cluster
                let ci = if let Some(&i) = cluster_idx.get(&cluster_name) {
                    i
                } else {
                    let i = self.data.swap_all_clusters.len();
                    self.data.swap_all_clusters.push(Cluster {
                        name: cluster_name.clone(),
                        hosts: Vec::new(),
                        resource_ids: Vec::new(),
                        state: ResourceState::Unknown,
                    });
                    cluster_idx.insert(cluster_name.clone(), i);
                    i
                };

                // O(1) find-or-create host
                let hi = if let Some(&i) = host_idx.get(&(ci, host_name.clone())) {
                    i
                } else {
                    let i = self.data.swap_all_clusters[ci].hosts.len();
                    self.data.swap_all_clusters[ci].hosts.push(Host {
                        name: host_name.clone(),
                        cpus: Vec::new(),
                        network_address: resource.network_address.as_deref().unwrap_or("").to_string(),
                        resource_ids: Vec::new(),
                        state: ResourceState::Unknown,
                    });
                    host_idx.insert((ci, host_name.clone()), i);
                    i
                };

                // O(1) find-or-create CPU
                let ki = if let Some(&i) = cpu_idx.get(&(ci, hi, cpu_name.clone())) {
                    i
                } else {
                    let i = self.data.swap_all_clusters[ci].hosts[hi].cpus.len();
                    self.data.swap_all_clusters[ci].hosts[hi].cpus.push(Cpu {
                        name: cpu_name.clone(),
                        resources: Vec::new(),
                        core_count: resource.core_count.unwrap_or(0) as i32,
                        cpufreq: resource.cpufreq.as_deref().unwrap_or("").parse::<f32>().unwrap_or(0.0),
                        chassis: resource.chassis.as_deref().unwrap_or("").to_string(),
                        resource_ids: Vec::new(),
                    });
                    cpu_idx.insert((ci, hi, cpu_name), i);
                    i
                };

                // Push resource and propagate resource_id up the hierarchy
                self.data.swap_all_clusters[ci].hosts[hi].cpus[ki].resources.push(Resource {
                    id: rid, state, thread_count: resource.thread_count.unwrap_or(0) as i32,
                });
                self.data.swap_all_clusters[ci].hosts[hi].cpus[ki].resource_ids.push(rid);
                self.data.swap_all_clusters[ci].hosts[hi].resource_ids.push(rid);
                self.data.swap_all_clusters[ci].resource_ids.push(rid);
            }

            for job in self.data.swap_all_jobs.iter_mut() {
                job.clusters = get_clusters_for_job(job, &self.data.swap_all_clusters);
                job.hosts = get_hosts_for_job(job, &self.data.swap_all_clusters);
                job.update_majority_resource_state(&self.data.swap_all_clusters);
            }

            for cluster in self.data.swap_all_clusters.iter_mut() {
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
                    host.state = if dead_count >= alive_count && dead_count >= absent_count {
                        ResourceState::Dead
                    } else if absent_count >= dead_count && absent_count >= alive_count {
                        ResourceState::Absent
                    } else if alive_count > dead_count && alive_count > absent_count {
                        ResourceState::Alive
                    } else {
                        ResourceState::Unknown
                    };
                }
            }

            for cluster in self.data.swap_all_clusters.iter_mut() {
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
                cluster.state = if dead_count >= alive_count && dead_count >= absent_count {
                    ResourceState::Dead
                } else if absent_count >= dead_count && absent_count >= alive_count {
                    ResourceState::Absent
                } else if alive_count > dead_count && alive_count > absent_count {
                    ResourceState::Alive
                } else {
                    ResourceState::Unknown
                };
            }

            let has_job_0 = self.data.all_jobs.iter().any(|job| job.id == 0);
            if has_job_0 {
                let job_0 = self.data.all_jobs.iter().find(|job| job.id == 0).unwrap().clone();
                self.data.swap_all_jobs.push(job_0);
            }

            self.data.all_jobs = self.data.swap_all_jobs.clone();
            self.data.all_clusters = self.data.swap_all_clusters.clone();
        }
    }

    pub fn check_dead_intervals_update(&mut self) {
        if !self.live_data { return; }
        if let Ok(intervals) = self.refresh.dead_intervals_receiver.try_recv() {
            self.data.dead_intervals = intervals;
        }
    }

    pub fn check_data_update(&mut self) {
        self.is_refreshing = *self.refresh.is_refreshing.lock().unwrap_or_else(|p| p.into_inner());
        self.live_refresh_paused = *self.refresh.refresh_rate.lock().unwrap_or_else(|p| p.into_inner()) == u64::MAX;

        self.check_job_update();
        self.check_ressource_update();
        self.check_dead_intervals_update();

        self.filters.set_scheduled_start_time(self.start_date.timestamp());
        self.filters.set_wall_time(self.end_date.timestamp());

        self.filter_jobs();
    }

    fn save_presets_to_file(&self, file_path: &str) {
        if let Ok(json) = serde_json::to_string(&self.prefs.cluster_presets) {
            let _ = std::fs::write(file_path, json);
        }
    }

    fn load_presets_from_file(file_path: &str) -> Vec<ClusterPreset> {
        match std::fs::read_to_string(file_path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    pub fn add_or_update_preset(&mut self, preset: ClusterPreset) {
        if let Some(existing) = self.prefs.cluster_presets.iter_mut().find(|p| p.name == preset.name) {
            *existing = preset;
        } else {
            self.prefs.cluster_presets.push(preset);
        }
        self.save_presets_to_file("presets.json");
    }

    pub fn remove_preset(&mut self, name: &str) {
        self.prefs.cluster_presets.retain(|p| p.name != name);
        self.save_presets_to_file("presets.json");
    }

    pub fn get_unique_owners(&self) -> Vec<String> {
        let mut owners: Vec<String> = self.data.all_jobs.iter().map(|job| job.owner.clone()).collect();
        owners.retain(|owner| owner != "all_resources");
        owners.sort();
        owners.dedup();
        owners
    }

    pub fn filter_jobs(&mut self) {
        let current_jobs = self.get_current_jobs();

        let selected_cluster_names: Option<Vec<String>> = self.filters.selected_preset.as_ref()
            .and_then(|preset_name| self.prefs.cluster_presets.iter().find(|p| p.name == *preset_name))
            .map(|preset| preset.clusters.clone());

        self.data.filtered_jobs = current_jobs
            .iter()
            .filter(|job| {
                job.id == 0
                    || (self.filters.owners.as_ref().map_or(true, |owners| owners.contains(&job.owner)))
                        && (self.filters.states.as_ref().map_or(true, |states| states.contains(&job.state)))
                        && (((self.filters.scheduled_start_time.map_or(true, |time| time <= job.scheduled_start))
                            && (self.filters.wall_time.map_or(true, |time| time >= job.scheduled_start)))
                            || ((self.filters.scheduled_start_time.map_or(true, |time| time <= job.get_end_date()))
                                && (self.filters.wall_time.map_or(true, |time| time >= job.get_end_date())))
                            || ((self.filters.scheduled_start_time.map_or(true, |time| time >= job.start_time))
                                && (self.filters.wall_time.map_or(true, |time| time <= job.get_end_date()))))
                        && (selected_cluster_names.is_none() || {
                            let cluster_names = selected_cluster_names.as_ref().unwrap();
                            cluster_names.iter().any(|cluster_name| job.clusters.contains(cluster_name))
                        })
            })
            .cloned()
            .collect();
    }

    pub fn get_current_jobs(&self) -> &[Job] {
        &self.data.all_jobs
    }

    pub fn get_current_clusters(&self) -> &Vec<Cluster> {
        &self.data.all_clusters
    }

    pub fn show_energy_diagram(&self) -> bool {
        self.show_energy
    }

    pub fn show_gantt(&self) -> bool {
        self.show_gantt_panel
    }

    pub fn current_file_type_supports_hierarchy(&self) -> bool {
        self.current_supports_hierarchy
    }

    pub fn get_current_energy_series(&self) -> Option<&[(i64, f64)]> {
        self.current_energy_series.first().map(|(_, s)| s.as_slice())
    }

    pub fn get_current_energy_series_multi(&self) -> Vec<(&str, &[(i64, f64)])> {
        self.current_energy_series.iter()
            .map(|(name, s)| (name.as_str(), s.as_slice()))
            .collect()
    }
}

impl Default for ApplicationContext {
    fn default() -> Self {
        let now: DateTime<Local> = Local::now();
        let mut context = Self {
            data: LiveDataState::default(),
            refresh: RefreshCoordinator::new(now),
            prefs: UiPreferences::default(),

            view_type: ViewType::Gantt,
            is_loading: false,
            filters: JobFilters::default(),
            live_data: false,

            start_date: now,
            end_date: now,
            is_refreshing: false,
            live_refresh_paused: false,
            refresh_requested: false,
            live_disable_requested: false,
            current_source_key: 0,
            current_source_name: String::new(),
            current_group_active: false,
            current_energy_series: Vec::new(),
            show_energy: true,
            show_gantt_panel: true,
            current_file_path: None,
            current_file_hash: None,
            current_file_type_name: String::new(),
            current_supports_hierarchy: true,
        };
        context.prefs.cluster_presets = ApplicationContext::load_presets_from_file("presets.json");
        // Center the initial live-data window on now using default_timespan so
        // the "now" line appears at the center of the Gantt on first load.
        let half = chrono::Duration::seconds(context.prefs.gantt_config.default_timespan_s / 2);
        context.start_date = now - half;
        context.end_date   = now + half;
        *context.refresh.start_date.lock().unwrap() = now - half;
        *context.refresh.end_date.lock().unwrap()   = now + half;
        context
    }
}

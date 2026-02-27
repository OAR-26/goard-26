use super::cluster::Cluster;
use super::filters::JobFilters;
use super::job::Job;
use super::resource::Resource;
use super::strata::Strata;
use crate::models::data_structure::cpu::Cpu;
use crate::models::data_structure::host::Host;
use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::utils::{get_clusters_for_job, get_hosts_for_job};
use crate::views::components::dashboard_components::job_table_sorting::JobSortable;
use crate::views::view::ViewType;
use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/*
`ApplicationContext` is the central state container for the entire application.
It manages jobs, clusters, resources, and application state, including filtering mechanisms
and communication channels for data updates.
*/
pub struct ApplicationContext {
    pub all_jobs: Vec<Job>,
    pub swap_all_jobs: Vec<Job>, // Used to store all jobs when refreshing (and swapped with all_jobs when refreshing is done)
    pub filtered_jobs: Vec<Job>, // Subset of all_jobs that match the filters

    pub all_clusters: Vec<Cluster>,
    pub swap_all_clusters: Vec<Cluster>, // Used to store all clusters when refreshing (and swapped with all_clusters when refreshing is done)

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

    // Latest resource metadata indexed by host (used for rich hover tooltips).
    pub strata_by_host: HashMap<String, Strata>,

    pub font_size: i32,
    pub see_all_jobs: bool,

    // UI requests (set by views, consumed by Menu/Options)
    pub theme_toggle_requested: bool,
}

impl ApplicationContext {
    pub fn check_job_update(&mut self) {
        if let Ok(new_jobs) = self.jobs_receiver.try_recv() {
            self.swap_all_jobs = new_jobs;
            self.is_loading = false;
        }
    }

    /*
    Checks for and processes any new resource data received from the background thread.
     This method builds the hierarchical structure of clusters, hosts, CPUs, and resources
     from the flat resource data received.
     */
    pub fn check_ressource_update(&mut self) {
        if let Ok(new_resources) = self.resources_receiver.try_recv() {
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

    pub fn check_data_update(&mut self) {
        self.check_job_update();
        self.check_ressource_update();

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
        self.filtered_jobs = self
            .all_jobs
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
                        && (self.filters.clusters.is_none() || {
                            let selected_clusters = self.filters.clusters.as_ref().unwrap();
                            selected_clusters.iter().any(|cluster| {
                                cluster.hosts.iter().any(|host| {
                                    host.cpus.iter().any(|cpu| {
                                        cpu.resources.iter().any(|resource| {
                                            job.assigned_resources.contains(&resource.id)
                                        })
                                    })
                                })
                            })
                        })
            })
            .cloned() // Clone filtred jobs here
            .collect();
    }
}

impl Default for ApplicationContext {
    // Creates a default ApplicationContext with initial values and sets up the background
    // data refresh mechanism.
    fn default() -> Self {
        let (jobs_sender, jobs_receiver) = channel();
        let (resources_sender, resources_receiver) = channel();

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
            user_connected: None,

            strata_by_host: HashMap::new(),

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

            theme_toggle_requested: false,
        };
        context.update_periodically();
        context
    }
}

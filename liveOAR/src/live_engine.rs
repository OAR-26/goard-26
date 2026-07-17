use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::HashMap;

use goard_core::models::data_structure::application_context::ApplicationContext;
use goard_core::models::data_structure::job::Job;
use goard_core::models::utils::utils::{clusters_for_job, hosts_for_job};

use crate::refresh_coordinator::RefreshCoordinator;

#[cfg(not(target_arch = "wasm32"))]
use crate::oar_fetch::{get_current_jobs_for_period, get_dead_intervals_from_json, get_jobs_from_json, get_resources_from_json};

#[cfg(target_arch = "wasm32")]
use crate::mocker::{mock_jobs, mock_stratas};

/// SSH host used to fetch live OAR data — read from the `GOARD_SSH_HOST`
/// environment variable. Falls back to `"grenoble.g5k"` when unset or empty.
#[cfg(not(target_arch = "wasm32"))]
fn load_ssh_host() -> String {
    std::env::var("GOARD_SSH_HOST")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "grenoble.g5k".to_string())
}

#[cfg(target_arch = "wasm32")]
fn load_ssh_host() -> String {
    "grenoble.g5k".to_string()
}

/// Owns the background-thread polling machinery that pulls live data from an
/// OAR cluster over SSH. `goard_core` has no notion of "live" — this engine
/// writes results directly into `ApplicationContext.data` once ready.
pub struct LiveEngine {
    pub refresh: RefreshCoordinator,
    ssh_host: String,
    /// Swap buffer — background thread writes here; promoted to app.data.all_jobs
    /// once the matching resource update has rebuilt the cluster hierarchy.
    swap_all_jobs: Vec<Job>,
}

impl LiveEngine {
    pub fn new(now: DateTime<Local>) -> Self {
        Self {
            refresh: RefreshCoordinator::new(now),
            ssh_host: load_ssh_host(),
            swap_all_jobs: Vec::new(),
        }
    }

    pub fn ssh_host(&self) -> &str {
        &self.ssh_host
    }

    pub fn set_ssh_host(&mut self, host: String) {
        self.ssh_host = host;
    }

    pub fn check_job_update(&mut self, app: &mut ApplicationContext) {
        if let Ok(new_jobs) = self.refresh.jobs_receiver.try_recv() {
            self.swap_all_jobs = new_jobs;
            app.is_loading = false;
        }
    }

    pub fn check_ressource_update(&mut self, app: &mut ApplicationContext) {
        let Ok(new_resources) = self.refresh.resources_receiver.try_recv() else { return; };

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
                    cpuset_by_host.entry(host.to_string()).or_default().extend(ints);
                }
            }
        }

        app.data.strata_by_resource_id.clear();
        for r in new_resources.iter() {
            if let Some(rid) = r.resource_id {
                app.data.strata_by_resource_id.insert(rid, r.clone());
            }
        }

        let now = chrono::Utc::now().timestamp();
        app.data.standby_upto.clear();
        for r in new_resources.iter() {
            if r.state.as_deref() == Some("Absent") {
                if let (Some(rid), Some(upto)) = (r.resource_id, r.available_upto) {
                    if upto > now && upto > 0 {
                        app.data.standby_upto.insert(rid, upto);
                    }
                }
            }
        }

        app.data.strata_by_host.clear();
        for r in new_resources.iter() {
            let host = r.host.as_deref().unwrap_or("").trim();
            let net = r.network_address.as_deref().unwrap_or("").trim();

            if !host.is_empty() {
                app.data.strata_by_host.entry(host.to_string()).or_insert_with(|| r.clone());
                let short = host.split('.').next().unwrap_or(host).trim();
                if !short.is_empty() {
                    app.data.strata_by_host.entry(short.to_string()).or_insert_with(|| r.clone());
                }
            }

            if !net.is_empty() {
                app.data.strata_by_host.entry(net.to_string()).or_insert_with(|| r.clone());
                let short = net.split('.').next().unwrap_or(net).trim();
                if !short.is_empty() {
                    app.data.strata_by_host.entry(short.to_string()).or_insert_with(|| r.clone());
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
                if let Some(existing) = app.data.strata_by_host.get(k).cloned() {
                    let existing_score = existing.comment.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                        + existing.cpuset.as_ref().map(non_empty_value).unwrap_or(false) as i32
                        + existing.cputype.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                        + existing.nodemodel.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32;
                    let new_score = r.comment.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                        + r.cpuset.as_ref().map(non_empty_value).unwrap_or(false) as i32
                        + r.cputype.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32
                        + r.nodemodel.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false) as i32;
                    if new_score > existing_score {
                        app.data.strata_by_host.insert(k.to_string(), r.clone());
                    }
                }
            }
        }

        for s in app.data.strata_by_host.values_mut() {
            let host_key = s.host.as_deref().unwrap_or("").trim();
            if host_key.is_empty() {
                continue;
            }
            if let Some(ints) = cpuset_by_host.get(host_key) {
                let mut ints = ints.clone();
                ints.sort_unstable();
                ints.dedup();
                if !ints.is_empty() {
                    let arr: Vec<Value> = ints.into_iter().map(|i| Value::Number(serde_json::Number::from(i))).collect();
                    s.cpuset = Some(Value::Array(arr));
                }
            }
        }

        // Rebuild cluster index maps from the freshly-populated strata.
        app.data.rebuild_cluster_index();

        for job in self.swap_all_jobs.iter_mut() {
            job.clusters = clusters_for_job(job, &app.data.strata_by_resource_id);
            job.hosts = hosts_for_job(job, &app.data.strata_by_resource_id);
            job.update_majority_resource_state(&app.data.strata_by_resource_id);
        }

        let has_job_0 = app.data.all_jobs.iter().any(|job| job.id == 0);
        if has_job_0 {
            let job_0 = app.data.all_jobs.iter().find(|job| job.id == 0).unwrap().clone();
            self.swap_all_jobs.push(job_0);
        }

        app.data.all_jobs = self.swap_all_jobs.clone();

        let (min_t, max_t) = app.data.all_jobs.iter()
            .filter(|j| j.id != 0 && j.start_time > 0)
            .fold(
                (i64::MAX, i64::MIN),
                |(mn, mx), j| {
                    let end = j.start_time + j.walltime;
                    (mn.min(j.start_time), mx.max(end))
                },
            );
        if min_t < max_t {
            let watts = app.prefs.gantt_config.energy_watts_per_resource;
            let estimated = crate::energy_estimate::estimate_from_jobs(
                &app.data.all_jobs, min_t, max_t, 10, watts,
            );
            app.data.plot_series = vec![("Estimated".to_string(), estimated)];
        }
    }

    pub fn check_dead_intervals_update(&mut self, app: &mut ApplicationContext) {
        if let Ok(intervals) = self.refresh.dead_intervals_receiver.try_recv() {
            app.data.dead_intervals = intervals;
        }
    }

    /// Drains pending background-thread results into `app.data`.
    pub fn poll(&mut self, app: &mut ApplicationContext) {
        self.check_job_update(app);
        self.check_ressource_update(app);
        self.check_dead_intervals_update(app);

        // Keep the background thread's polling window in sync with the current view.
        *self.refresh.start_date.lock().unwrap() = app.start_date;
        *self.refresh.end_date.lock().unwrap() = app.end_date;
    }

    /// Current refresh rate in seconds (`u64::MAX` = paused/never).
    pub fn refresh_rate(&self) -> u64 {
        *self.refresh.refresh_rate.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Set the refresh rate in seconds (`u64::MAX` = paused/never).
    pub fn set_refresh_rate(&self, rate_s: u64) {
        *self.refresh.refresh_rate.lock().unwrap() = rate_s;
    }

    /// Whether a background fetch is currently in flight.
    pub fn is_refreshing(&self) -> bool {
        *self.refresh.is_refreshing.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn instant_update(&mut self, app: &mut ApplicationContext) {
        let is_refreshing = self.refresh.is_refreshing.clone();

        if *is_refreshing.lock().unwrap() {
            return;
        }
        *is_refreshing.lock().unwrap() = true;

        // Read straight from `app` rather than the mutex: `poll()` syncs the
        // mutex from `app.start_date`/`end_date` at the *start* of the frame,
        // before any navigation (pan/zoom/jump) happens during render. Using
        // the mutex here would fetch the window the user just navigated away
        // from. Re-sync the mutex too, so the periodic background thread
        // also picks up the latest window.
        let start = app.start_date;
        let end = app.end_date;
        *self.refresh.start_date.lock().unwrap() = start;
        *self.refresh.end_date.lock().unwrap() = end;

        let jobs_sender = self.refresh.jobs_sender.clone();
        let resources_sender = self.refresh.resources_sender.clone();
        let dead_intervals_sender = self.refresh.dead_intervals_sender.clone();
        let ssh_host = self.ssh_host.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let is_refreshing_clone = is_refreshing.clone();
            thread::spawn(move || {
                let res = get_current_jobs_for_period(start, end, &ssh_host, "./liveOAR/data/data.json");
                if res {
                    let jobs = get_jobs_from_json("./liveOAR/data/data.json");
                    let resources = get_resources_from_json("./liveOAR/data/data.json");
                    let dead_intervals = get_dead_intervals_from_json("./liveOAR/data/data.json");
                    jobs_sender.send(jobs).unwrap_or_else(|e| println!("Error while sending jobs: {}", e));
                    resources_sender.send(resources).unwrap_or_else(|e| println!("Error while sending resources: {}", e));
                    dead_intervals_sender.send(dead_intervals).unwrap_or_else(|e| println!("Error while sending dead intervals: {}", e));
                }
                *is_refreshing_clone.lock().unwrap() = false;
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let is_refreshing = is_refreshing.clone();
            let start_ts = start.timestamp();
            let end_ts = end.timestamp();
            wasm_bindgen_futures::spawn_local(async move {
                let snap = refresh_snapshot(start_ts, end_ts).await;
                match snap {
                    Some(s) => {
                        jobs_sender.send(s.jobs).ok();
                        resources_sender.send(s.resources).ok();
                    }
                    None => {
                        jobs_sender.send(mock_jobs()).ok();
                        resources_sender.send(mock_stratas()).ok();
                    }
                }
                *is_refreshing.lock().unwrap() = false;
            });
        }
    }

    pub fn update_periodically(&mut self, app: &mut ApplicationContext) {
        if self.refresh.thread_started {
            *self.refresh.refresh_rate.lock().unwrap() = 30;
            return;
        }
        self.refresh.thread_started = true;
        let refresh_rate = self.refresh.refresh_rate.clone();
        let jobs_sender = self.refresh.jobs_sender.clone();
        let resources_sender = self.refresh.resources_sender.clone();
        let dead_intervals_sender = self.refresh.dead_intervals_sender.clone();
        let is_refreshing = self.refresh.is_refreshing.clone();
        let start_date = self.refresh.start_date.clone();
        let end_date = self.refresh.end_date.clone();
        let ssh_host = self.ssh_host.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            thread::spawn(move || loop {
                let rate = *refresh_rate.lock().unwrap();

                if rate == u64::MAX {
                    thread::sleep(Duration::from_secs(5));
                    continue;
                }

                if *is_refreshing.lock().unwrap() {
                    thread::sleep(Duration::from_secs(rate));
                    continue;
                }

                *is_refreshing.lock().unwrap() = true;

                let start = *start_date.lock().unwrap();
                let end = *end_date.lock().unwrap();

                let res = get_current_jobs_for_period(start, end, &ssh_host, "./liveOAR/data/data.json");
                if res {
                    let jobs = get_jobs_from_json("./liveOAR/data/data.json");
                    let resources = get_resources_from_json("./liveOAR/data/data.json");
                    let dead_intervals = get_dead_intervals_from_json("./liveOAR/data/data.json");

                    jobs_sender.send(jobs).unwrap_or_else(|e| println!("Error while sending jobs: {}", e));
                    resources_sender.send(resources).unwrap_or_else(|e| println!("Error while sending resources: {}", e));
                    dead_intervals_sender.send(dead_intervals).unwrap_or_else(|e| println!("Error while sending dead intervals: {}", e));
                }

                *is_refreshing.lock().unwrap() = false;
                thread::sleep(Duration::from_secs(rate));
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                loop {
                    let start = start_date.lock().unwrap().timestamp();
                    let end = end_date.lock().unwrap().timestamp();
                    let snap = fetch_snapshot(start, end).await;
                    match snap {
                        Some(s) => {
                            jobs_sender.send(s.jobs).ok();
                            resources_sender.send(s.resources).ok();
                        }
                        None => {
                            jobs_sender.send(mock_jobs()).ok();
                            resources_sender.send(mock_stratas()).ok();
                        }
                    }
                    gloo_timers::future::TimeoutFuture::new(30_000).await;
                }
            });
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_snapshot(start: i64, end: i64) -> Option<crate::api_types::ApiSnapshot> {
    let url = format!("/api/data?start={}&end={}", start, end);
    let resp = gloo_net::http::Request::get(&url).send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    resp.json::<crate::api_types::ApiSnapshot>().await.ok()
}

#[cfg(target_arch = "wasm32")]
async fn refresh_snapshot(start: i64, end: i64) -> Option<crate::api_types::ApiSnapshot> {
    let url = format!("/api/refresh?start={}&end={}", start, end);
    let resp = gloo_net::http::Request::post(&url).send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    resp.json::<crate::api_types::ApiSnapshot>().await.ok()
}

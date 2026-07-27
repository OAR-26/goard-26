use goard_core::models::data_structure::job::{Job, JobState};
use goard_core::models::data_structure::resource::{DeadInterval, ResourceState};
use goard_core::models::data_structure::strata::Strata;

use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::process::Command;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use chrono::{DateTime, Local};

// ---------------------------------------------------------------------------
// OarVersion trait - full contract for one OAR version
// ---------------------------------------------------------------------------

pub trait OarVersion: Send + Sync {
    /// SSH command to run on the cluster for the given time window.
    /// `start` and `end` are already formatted as `"YYYY-MM-DD HH:MM:SS"`.
    fn oarstat_command(&self, start: &str, end: &str) -> String;

    fn parse_job(&self, json: &Value) -> Job;
    fn parse_resource(&self, json: &Value) -> Option<Strata>;
}

// ---------------------------------------------------------------------------
// OAR 2
// ---------------------------------------------------------------------------

pub struct Oar2;

impl OarVersion for Oar2 {
    fn oarstat_command(&self, start: &str, end: &str) -> String {
        format!("oarstat -J -g \"{}, {}\"", start, end)
    }

    fn parse_job(&self, json: &Value) -> Job {
        let queue = json
            .get("queue_name")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("queue").and_then(|v| v.as_str()))
            .unwrap_or("default")
            .to_string();

        Job {
            id: json["id"]
                .as_str()
                .unwrap_or("0")
                .parse::<u32>()
                .unwrap_or(0),
            owner: json["owner"].as_str().unwrap_or("unknown").to_string(),
            state: parse_state(json["state"].as_str().unwrap_or("unknown")),
            command: json["command"].as_str().unwrap_or("").to_string(),
            walltime: json["walltime"].as_i64().unwrap_or(0),
            message: json["message"].as_str().map(|s| s.to_string()),
            queue,
            assigned_resources: json["resource_id"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_str().and_then(|s| s.parse::<u32>().ok()))
                .collect(),
            scheduled_start: json["start_time"].as_i64().unwrap_or(0),
            start_time: json["start_time"].as_i64().unwrap_or(0),
            stop_time: json["stop_time"].as_i64().unwrap_or(0),
            submission_time: json["submission_time"].as_i64().unwrap_or(0),
            exit_code: json["exit_code"].as_i64().map(|n| n as i32),
            clusters: Vec::new(),
            hosts: Vec::new(),
            main_resource_state: ResourceState::Unknown,
            job_type: json["type"].as_str().unwrap_or("").to_string(),
            job_types: json["types"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            name: json["name"].as_str().map(|s| s.to_string()),
            project: json["project"].as_str().unwrap_or("").to_string(),
        }
    }

    fn parse_resource(&self, json: &Value) -> Option<Strata> {
        serde_json::from_value(json.clone()).ok()
    }
}

// ---------------------------------------------------------------------------
// OAR 3
// ---------------------------------------------------------------------------

pub struct Oar3;

impl OarVersion for Oar3 {
    fn oarstat_command(&self, start: &str, end: &str) -> String {
        format!("oarstat -J -g \"{}, {}\"", start, end)
    }

    fn parse_job(&self, json: &Value) -> Job {
        let queue = json
            .get("queue_name")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        // OAR3: "types" is a comma-separated string, not an array
        let job_types: Vec<String> = json["types"]
            .as_str()
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let job_type = job_types.first().cloned().unwrap_or_default();

        Job {
            // OAR3: id is an integer, not a string
            id: json["id"].as_u64().unwrap_or(0) as u32,
            // OAR3: "user" instead of "owner"
            owner: json["user"].as_str().unwrap_or("unknown").to_string(),
            state: parse_state(json["state"].as_str().unwrap_or("unknown")),
            command: json["command"].as_str().unwrap_or("").to_string(),
            walltime: json["walltime"].as_i64().unwrap_or(0),
            message: json["message"].as_str().map(|s| s.to_string()),
            queue,
            // OAR3: "assigned_resources" with integer values, not "resource_id" with strings
            assigned_resources: json["assigned_resources"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u32))
                .collect(),
            scheduled_start: json["start_time"].as_i64().unwrap_or(0),
            start_time: json["start_time"].as_i64().unwrap_or(0),
            stop_time: json["stop_time"].as_i64().unwrap_or(0),
            submission_time: json["submission_time"].as_i64().unwrap_or(0),
            exit_code: json["exit_code"].as_i64().map(|n| n as i32),
            clusters: Vec::new(),
            hosts: Vec::new(),
            main_resource_state: ResourceState::Unknown,
            job_type,
            job_types,
            name: json["name"].as_str().map(|s| s.to_string()),
            project: json["project"].as_str().unwrap_or("").to_string(),
        }
    }

    fn parse_resource(&self, json: &Value) -> Option<Strata> {
        let mut strata: Strata = serde_json::from_value(json.clone()).ok()?;
        // OAR3: "id" instead of "resource_id"
        if strata.resource_id.is_none() {
            strata.resource_id = json.get("id").and_then(|v| v.as_u64()).map(|v| v as u32);
        }
        Some(strata)
    }
}

// ---------------------------------------------------------------------------
// Factory - reads GOARD_OAR_VERSION env var (default: "2")
// ---------------------------------------------------------------------------

pub fn make_oar_version() -> Arc<dyn OarVersion> {
    match std::env::var("GOARD_OAR_VERSION").as_deref() {
        Ok("3") => Arc::new(Oar3),
        _       => Arc::new(Oar2),
    }
}

// ---------------------------------------------------------------------------
// SSH fetch
// ---------------------------------------------------------------------------

pub fn test_connection(host: &str) -> Result<(), String> {
    let ssh_test = Command::new("ssh").args([host, "true"]).status();
    match ssh_test {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("SSH command failed with status: {}", status)),
        Err(e) => Err(format!("Connection test failed: {}", e)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_current_jobs_for_period(
    start_date: DateTime<Local>,
    end_date: DateTime<Local>,
    ssh_host: &str,
    output_path: &str,
    version: &dyn OarVersion,
) -> bool {
    let interval = end_date - start_date;
    let margin = interval.num_seconds() * 30 / 100;
    let start_date = start_date - chrono::Duration::seconds(margin);
    let end_date = end_date + chrono::Duration::seconds(margin);

    if test_connection(ssh_host) != Ok(()) {
        return false;
    }

    if let Some(parent) = std::path::Path::new(output_path).parent() {
        if !parent.exists() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let cmd = version.oarstat_command(
        &start_date.format("%Y-%m-%d %H:%M:%S").to_string(),
        &end_date.format("%Y-%m-%d %H:%M:%S").to_string(),
    );

    let ssh_status = Command::new("ssh")
        .args([ssh_host, &cmd])
        .output()
        .and_then(|output| std::fs::write(output_path, output.stdout));

    if let Err(e) = ssh_status {
        println!("Failed to execute SSH command: {}", e);
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// JSON readers
// ---------------------------------------------------------------------------

pub fn get_jobs_from_json(file_path: &str, version: &dyn OarVersion) -> Vec<Job> {
    let json = match read_json(file_path) {
        Some(v) => v,
        None => return Vec::new(),
    };
    let mut jobs = Vec::new();
    if let Some(Value::Object(map)) = json.get("jobs") {
        for (_, value) in map {
            jobs.push(version.parse_job(value));
        }
    }
    jobs
}

pub fn get_resources_from_json(file_path: &str, version: &dyn OarVersion) -> Vec<Strata> {
    let json = match read_json(file_path) {
        Some(v) => v,
        None => return Vec::new(),
    };
    let mut resources = Vec::new();
    if let Some(arr) = json.get("resources").and_then(|v| v.as_array()) {
        for value in arr {
            if let Some(strata) = version.parse_resource(value) {
                resources.push(strata);
            }
        }
    }
    resources
}

pub fn get_dead_intervals_from_json(file_path: &str) -> HashMap<u32, Vec<DeadInterval>> {
    let json = match read_json(file_path) {
        Some(v) => v,
        None => return HashMap::new(),
    };
    let mut result: HashMap<u32, Vec<DeadInterval>> = HashMap::new();
    if let Some(dead) = json.get("dead_resources").and_then(|v| v.as_object()) {
        for (id_str, intervals) in dead {
            let id: u32 = match id_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let mut ivs = Vec::new();
            if let Some(arr) = intervals.as_array() {
                for iv in arr {
                    if let Some(iv_arr) = iv.as_array() {
                        if iv_arr.len() >= 3 {
                            let start_s = iv_arr[0].as_i64().unwrap_or(0);
                            let end_s = iv_arr[1].as_i64().unwrap_or(0);
                            let state = match iv_arr[2].as_str().unwrap_or("") {
                                "Dead" => ResourceState::Dead,
                                "Absent" => ResourceState::Absent,
                                "Suspected" => ResourceState::Suspected,
                                _ => ResourceState::Unknown,
                            };
                            if state != ResourceState::Unknown {
                                ivs.push(DeadInterval { start_s, end_s, state });
                            }
                        }
                    }
                }
            }
            if !ivs.is_empty() {
                result.insert(id, ivs);
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn parse_state(s: &str) -> JobState {
    serde_json::from_str(&format!("\"{}\"", s)).unwrap_or(JobState::Unknown)
}

pub fn parse_state_from_json(json_str: &str) -> Result<JobState, serde_json::Error> {
    serde_json::from_str(json_str)
}

fn read_json(file_path: &str) -> Option<Value> {
    let mut file = File::open(file_path)
        .map_err(|e| println!("Unable to open {}: {}", file_path, e))
        .ok()?;
    let mut data = String::new();
    file.read_to_string(&mut data)
        .map_err(|e| println!("Unable to read {}: {}", file_path, e))
        .ok()?;
    serde_json::from_str(&data)
        .map_err(|e| println!("Unable to parse JSON {}: {}", file_path, e))
        .ok()
}

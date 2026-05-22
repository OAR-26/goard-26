use super::{FileTypeConfig, ParsedFileData, ValidationError, VisualizationTarget};
use crate::models::data_structure::{
    cluster::Cluster,
    cpu::Cpu,
    host::Host,
    job::{Job, JobState},
    resource::{Resource, ResourceState},
    strata::Strata,
};
use crate::models::utils::utils::{
    get_all_clusters, get_all_hosts, get_all_resources,
    get_clusters_for_job, get_hosts_for_job,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

// ── JSON config schema ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OarDetectionConfig {
    required_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OarSchemaConfig {
    resources_key: String,
    jobs_key: String,
}

#[derive(Debug, Deserialize)]
struct OarValidationRule {
    field: String,
    expected_type: String,
    required: bool,
}

#[derive(Debug, Deserialize)]
struct OarValidationConfig {
    rules: Vec<OarValidationRule>,
}

#[derive(Debug, Deserialize)]
struct OarStateMappings {
    job_states: HashMap<String, String>,
    resource_states: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct OarConfig {
    name: String,
    description: String,
    detection: OarDetectionConfig,
    visualizations: Vec<VisualizationTarget>,
    schema: OarSchemaConfig,
    validation: OarValidationConfig,
    job_field_mappings: HashMap<String, String>,
    state_mappings: OarStateMappings,
}

// Embedded at compile time — no runtime file path needed.
const OAR_CONFIG_JSON: &str = include_str!("../../../file_types/oar.json");

// ── OarFileType ───────────────────────────────────────────────────────────────

pub struct OarFileType {
    config: OarConfig,
    visualizations: Vec<VisualizationTarget>,
}

impl OarFileType {
    pub fn new() -> Self {
        let config: OarConfig =
            serde_json::from_str(OAR_CONFIG_JSON).expect("file_types/oar.json is malformed");
        let visualizations = config.visualizations.clone();
        Self { config, visualizations }
    }

    fn job_field(&self, key: &str) -> String {
        self.config.job_field_mappings.get(key).cloned().unwrap_or_else(|| key.to_string())
    }

    fn parse_job_state(&self, raw: &str) -> JobState {
        match self
            .config
            .state_mappings
            .job_states
            .get(raw)
            .map(|s| s.as_str())
            .unwrap_or(raw)
        {
            "Running" => JobState::Running,
            "Waiting" => JobState::Waiting,
            "Terminated" => JobState::Terminated,
            "Error" => JobState::Error,
            _ => JobState::Unknown,
        }
    }

    fn parse_resource_state(&self, raw: &str) -> ResourceState {
        match self
            .config
            .state_mappings
            .resource_states
            .get(raw)
            .map(|s| s.as_str())
            .unwrap_or(raw)
        {
            "Alive" => ResourceState::Alive,
            "Dead" => ResourceState::Dead,
            "Absent" => ResourceState::Absent,
            "Suspected" => ResourceState::Suspected,
            _ => ResourceState::Unknown,
        }
    }

    fn parse_resources(&self, json: &Value) -> Result<Vec<Strata>, String> {
        let key = &self.config.schema.resources_key;
        let Some(val) = json.get(key) else { return Ok(Vec::new()) };
        let Some(arr) = val.as_array() else {
            return Err(format!("'{}' must be an array", key));
        };
        arr.iter()
            .map(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| format!("resource parse error: {}", e))
            })
            .collect()
    }

    fn build_clusters(&self, resources: &[Strata]) -> Vec<Cluster> {
        let mut clusters: Vec<Cluster> = Vec::new();

        for r in resources {
            let cluster_name = r.cluster.as_deref().unwrap_or("").trim().to_string();
            if cluster_name.is_empty() {
                continue;
            }

            let rid = r.resource_id.unwrap_or(0);
            let host_name = r.host.as_deref().unwrap_or("").to_string();
            let net_addr = r.network_address.as_deref().unwrap_or("").to_string();
            let res_state = self.parse_resource_state(r.state.as_deref().unwrap_or(""));
            let thread_count = r.thread_count.unwrap_or(0) as i32;

            let new_resource = Resource { id: rid, state: res_state, thread_count };
            let new_cpu = Cpu {
                name: r.cputype.as_deref().unwrap_or("").to_string(),
                resources: vec![new_resource],
                core_count: r.core_count.unwrap_or(0) as i32,
                cpufreq: r.cpufreq.as_deref().unwrap_or("0").parse::<f32>().unwrap_or(0.0),
                chassis: r.chassis.as_deref().unwrap_or("").to_string(),
                resource_ids: vec![rid],
            };
            let new_host = Host {
                name: host_name.clone(),
                cpus: vec![new_cpu],
                network_address: net_addr,
                resource_ids: vec![rid],
                state: ResourceState::Unknown,
            };

            if let Some(cluster) = clusters.iter_mut().find(|c| c.name == cluster_name) {
                cluster.resource_ids.push(rid);
                if let Some(host) = cluster.hosts.iter_mut().find(|h| h.name == host_name) {
                    host.resource_ids.push(rid);
                    if let Some(cpu) = host.cpus.first_mut() {
                        cpu.resource_ids.push(rid);
                        cpu.resources.push(Resource { id: rid, state: res_state, thread_count });
                    }
                } else {
                    cluster.hosts.push(new_host);
                }
            } else {
                clusters.push(Cluster {
                    name: cluster_name,
                    hosts: vec![new_host],
                    resource_ids: vec![rid],
                    state: ResourceState::Unknown,
                });
            }
        }

        clusters
    }

    fn parse_single_job(&self, data: &Value) -> Result<Job, String> {
        macro_rules! field {
            ($key:expr) => { self.job_field($key) };
        }

        let id = data
            .get(field!("id").as_str())
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0) as u32;

        let owner = data
            .get(field!("owner").as_str())
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let state = self.parse_job_state(
            data.get(field!("state").as_str()).and_then(|v| v.as_str()).unwrap_or("Unknown"),
        );

        let start_time = data.get(field!("start_time").as_str()).and_then(|v| v.as_i64()).unwrap_or(0);
        let walltime = data.get(field!("walltime").as_str()).and_then(|v| v.as_i64()).unwrap_or(0);
        let stop_time = data
            .get(field!("stop_time").as_str())
            .and_then(|v| v.as_i64())
            .unwrap_or(start_time + walltime);
        let submission_time =
            data.get(field!("submission_time").as_str()).and_then(|v| v.as_i64()).unwrap_or(start_time);

        let command =
            data.get(field!("command").as_str()).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let queue = data.get(field!("queue").as_str()).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let job_type =
            data.get(field!("job_type").as_str()).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let name = data.get(field!("name").as_str()).and_then(|v| v.as_str()).map(str::to_string);
        let project =
            data.get(field!("project").as_str()).and_then(|v| v.as_str()).unwrap_or("").to_string();

        let job_types = data
            .get(field!("job_types").as_str())
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();

        let hosts = data
            .get(field!("hosts").as_str())
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();

        // Extract cluster from properties field: cluster='<name>'
        let clusters = data
            .get(field!("properties").as_str())
            .and_then(|v| v.as_str())
            .and_then(|props| {
                let start = props.find("cluster='")?;
                let rest = &props[start + 9..];
                let end = rest.find('\'')?;
                Some(vec![rest[..end].to_string()])
            })
            .unwrap_or_default();

        let assigned_resources = data
            .get(field!("assigned_resources").as_str())
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str()?.parse::<u32>().ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(Job {
            id,
            owner,
            state,
            scheduled_start: start_time,
            walltime,
            hosts,
            clusters,
            command,
            message: None,
            queue,
            assigned_resources,
            submission_time,
            start_time,
            stop_time,
            exit_code: None,
            main_resource_state: ResourceState::Alive,
            job_type,
            job_types,
            name,
            project,
        })
    }

    fn parse_jobs(&self, json: &Value, clusters: &Vec<Cluster>) -> Result<Vec<Job>, String> {
        let key = &self.config.schema.jobs_key;
        let Some(val) = json.get(key) else { return Ok(Vec::new()) };
        let Some(map) = val.as_object() else {
            return Err(format!("'{}' must be an object", key));
        };

        let mut jobs = Vec::new();
        let mut next_synthetic_id: u32 = u32::MAX;

        for (id_str, job_data) in map {
            let mut job = self.parse_single_job(job_data)?;

            if job.id == 0 {
                if let Ok(parsed) = id_str.parse::<u32>() {
                    job.id = parsed;
                } else {
                    job.id = next_synthetic_id;
                    next_synthetic_id = next_synthetic_id.saturating_sub(1);
                }
            }

            jobs.push(job);
        }

        for job in jobs.iter_mut() {
            job.clusters = get_clusters_for_job(job, clusters);
            job.hosts = get_hosts_for_job(job, clusters);
            job.update_majority_resource_state(clusters);
        }

        // Pseudo-job representing all resources (mirrors live data behavior).
        jobs.push(Job {
            id: 0,
            owner: "all_resources".to_string(),
            state: JobState::Unknown,
            scheduled_start: 0,
            walltime: 0,
            hosts: get_all_hosts(clusters),
            clusters: get_all_clusters(clusters),
            command: String::new(),
            message: None,
            queue: String::new(),
            assigned_resources: get_all_resources(clusters),
            submission_time: 0,
            start_time: 0,
            stop_time: 0,
            exit_code: None,
            main_resource_state: ResourceState::Unknown,
            job_type: String::new(),
            job_types: Vec::new(),
            name: None,
            project: String::new(),
        });

        Ok(jobs)
    }
}

// ── FileTypeConfig impl ───────────────────────────────────────────────────────

impl FileTypeConfig for OarFileType {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn visualization_targets(&self) -> &[VisualizationTarget] {
        &self.visualizations
    }

    fn detect(&self, content: &str) -> f32 {
        let Ok(json) = serde_json::from_str::<Value>(content) else {
            return 0.0;
        };
        let required = &self.config.detection.required_fields;
        let matched = required.iter().filter(|f| json.get(f.as_str()).is_some()).count();
        matched as f32 / required.len().max(1) as f32
    }

    fn validate(&self, content: &str) -> Vec<ValidationError> {
        let json = match serde_json::from_str::<Value>(content) {
            Ok(j) => j,
            Err(e) => {
                return vec![ValidationError {
                    field: "root".into(),
                    message: format!("invalid JSON: {}", e),
                }]
            }
        };

        let mut errors = Vec::new();
        for rule in &self.config.validation.rules {
            match json.get(&rule.field) {
                None if rule.required => {
                    errors.push(ValidationError {
                        field: rule.field.clone(),
                        message: "required field missing".into(),
                    });
                }
                Some(v) => {
                    let ok = match rule.expected_type.as_str() {
                        "array" => v.is_array(),
                        "object" => v.is_object(),
                        "string" => v.is_string(),
                        _ => true,
                    };
                    if !ok {
                        errors.push(ValidationError {
                            field: rule.field.clone(),
                            message: format!("expected {}", rule.expected_type),
                        });
                    }
                }
                _ => {}
            }
        }
        errors
    }

    fn parse(&self, content: &str) -> Result<ParsedFileData, String> {
        let json: Value =
            serde_json::from_str(content).map_err(|e| format!("JSON parse error: {}", e))?;

        let resources = self.parse_resources(&json)?;

        let strata_by_resource_id: HashMap<u32, Strata> = resources
            .iter()
            .filter_map(|r| r.resource_id.map(|id| (id, r.clone())))
            .collect();

        let clusters = self.build_clusters(&resources);
        let jobs = self.parse_jobs(&json, &clusters)?;

        Ok(ParsedFileData { resources, clusters, jobs, strata_by_resource_id, raw_energy_series: None })
    }
}

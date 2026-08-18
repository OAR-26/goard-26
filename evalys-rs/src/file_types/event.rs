use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

use ganttza::models::data_structure::{marker::{GanttMarker, MarkerShape}, strata::Strata};

use super::{FileTypeConfig, ParsedFileData, ValidationError, VisualizationTarget};

pub struct EventFileType;

impl EventFileType {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Deserialize)]
struct RawEvent {
    timestamp: i64,
    resource: String,
    event: String,
    #[allow(dead_code)]
    walltime: Option<i64>,
}

impl FileTypeConfig for EventFileType {
    fn name(&self) -> &str {
        "Event"
    }

    fn description(&self) -> &str {
        "JSON array of timed events (start/stop circles) on named resources"
    }

    fn visualization_targets(&self) -> &[VisualizationTarget] {
        &[VisualizationTarget::Gantt]
    }

    fn supports_hierarchy_controls(&self) -> bool {
        false
    }

    fn hierarchy_levels(&self) -> Option<Vec<String>> {
        // One flat level: the resource name (stored as "host" in synthesized strata).
        Some(vec!["host".to_string()])
    }

    fn detect(&self, content: &str) -> f32 {
        let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(content) else {
            return 0.0;
        };
        if arr.is_empty() {
            return 0.0;
        }
        let sample = arr.len().min(5);
        let hits = arr.iter().take(sample).filter(|v| {
            v.get("timestamp").is_some()
                && v.get("resource").is_some()
                && v.get("event").is_some()
        }).count();
        if hits == 0 { 0.0 } else { hits as f32 / sample as f32 }
    }

    fn validate(&self, content: &str) -> Vec<ValidationError> {
        let arr: Vec<serde_json::Value> = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(e) => return vec![ValidationError {
                field: "json".to_string(),
                message: e.to_string(),
            }],
        };
        let mut errors = Vec::new();
        for (i, v) in arr.iter().enumerate() {
            for field in &["timestamp", "resource", "event"] {
                if v.get(field).is_none() {
                    errors.push(ValidationError {
                        field: format!("[{}].{}", i, field),
                        message: "missing".to_string(),
                    });
                }
            }
        }
        errors
    }

    fn parse(&self, content: &str) -> Result<ParsedFileData, String> {
        let raw: Vec<RawEvent> = serde_json::from_str(content)
            .map_err(|e| format!("Parse error: {}", e))?;

        // Collect unique resource names in stable order → synthesize one Strata row each.
        let mut seen: BTreeMap<String, u32> = BTreeMap::new();
        let mut next_id: u32 = 1;
        for ev in &raw {
            seen.entry(ev.resource.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
        }

        let mut strata_by_resource_id = HashMap::new();
        let resources: Vec<Strata> = seen.iter().map(|(name, &id)| {
            let s = Strata {
                host: Some(name.clone()),
                resource_id: Some(id),
                state: Some("Alive".to_string()),
                ..Strata::default()
            };
            strata_by_resource_id.insert(id, s.clone());
            s
        }).collect();

        let markers = raw.into_iter().map(|ev| {
            let lower = ev.event.to_lowercase();
            let (color, label): ([u8; 4], &str) = match lower.as_str() {
                "start" => ([0, 200, 80, 220], "start"),
                "stop"  => ([220, 40, 40, 220], "stop"),
                other   => ([180, 180, 180, 200], other),
            };
            GanttMarker {
                timestamp_s: ev.timestamp,
                resource_name: ev.resource.clone(),
                shape: MarkerShape::Circle,
                color,
                tooltip: Some(format!("{}: {} @ {}", ev.resource, label, ev.timestamp)),
            }
        }).collect();

        Ok(ParsedFileData {
            resources,
            jobs: Vec::new(),
            strata_by_resource_id,
            raw_energy_series: None,
            markers,
        })
    }
}

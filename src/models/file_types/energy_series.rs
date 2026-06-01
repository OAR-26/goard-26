use super::{FileTypeConfig, ParsedFileData, ValidationError, VisualizationTarget};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

// ── JSON config schema ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DetectionConfig {
    required_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SchemaConfig {
    timestamp_field: String,
    energy_field: String,
    walltime_field: String,
}

#[derive(Debug, Deserialize)]
struct EnergySeriesConfig {
    name: String,
    description: String,
    detection: DetectionConfig,
    visualizations: Vec<VisualizationTarget>,
    schema: SchemaConfig,
}

const CONFIG_JSON: &str = include_str!("../../../file_types/energy_series.json");

// ── EnergySeriesFileType ──────────────────────────────────────────────────────

pub struct EnergySeriesFileType {
    config: EnergySeriesConfig,
    visualizations: Vec<VisualizationTarget>,
}

impl EnergySeriesFileType {
    pub fn new() -> Self {
        let config: EnergySeriesConfig =
            serde_json::from_str(CONFIG_JSON).expect("file_types/energy_series.json is malformed");
        let visualizations = config.visualizations.clone();
        Self { config, visualizations }
    }
}

impl FileTypeConfig for EnergySeriesFileType {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn visualization_targets(&self) -> &[VisualizationTarget] {
        &self.visualizations
    }

    /// Matches JSON arrays whose first element contains all required fields.
    fn detect(&self, content: &str) -> f32 {
        let Ok(val) = serde_json::from_str::<Value>(content) else {
            return 0.0;
        };
        let Some(arr) = val.as_array() else {
            return 0.0;
        };
        let Some(first) = arr.first() else {
            return 0.0;
        };
        let required = &self.config.detection.required_fields;
        let matched = required.iter().filter(|f| first.get(f.as_str()).is_some()).count();
        matched as f32 / required.len().max(1) as f32
    }

    fn validate(&self, content: &str) -> Vec<ValidationError> {
        let val = match serde_json::from_str::<Value>(content) {
            Ok(v) => v,
            Err(e) => {
                return vec![ValidationError {
                    field: "root".into(),
                    message: format!("invalid JSON: {}", e),
                }]
            }
        };
        let mut errors = Vec::new();
        if !val.is_array() {
            errors.push(ValidationError {
                field: "root".into(),
                message: "expected a JSON array".into(),
            });
            return errors;
        }
        let arr = val.as_array().unwrap();
        for field in &self.config.detection.required_fields {
            if arr.first().and_then(|v| v.get(field.as_str())).is_none() {
                errors.push(ValidationError {
                    field: field.clone(),
                    message: "required field missing in first array element".into(),
                });
            }
        }
        errors
    }

    fn parse(&self, content: &str) -> Result<ParsedFileData, String> {
        let val: Value =
            serde_json::from_str(content).map_err(|e| format!("JSON parse error: {}", e))?;
        let arr = val.as_array().ok_or("Expected JSON array at root")?;

        let ts_field = &self.config.schema.timestamp_field;
        let energy_field = &self.config.schema.energy_field;

        let mut series: Vec<(i64, f64)> = arr
            .iter()
            .filter_map(|item| {
                let ts = item.get(ts_field.as_str())?.as_i64()?;
                let energy = item.get(energy_field.as_str())?.as_f64()?;
                Some((ts, energy))
            })
            .collect();

        series.sort_by_key(|(ts, _)| *ts);

        Ok(ParsedFileData {
            resources: Vec::new(),
            clusters: Vec::new(),
            jobs: Vec::new(),
            strata_by_resource_id: HashMap::new(),
            raw_energy_series: Some(series),
            markers: Vec::new(),
        })
    }
}

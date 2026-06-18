use goard_core::models::data_structure::{cluster::Cluster, job::Job, marker::GanttMarker, strata::Strata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod energy_series;
pub mod event;
pub mod oar;

// ── Visualization targets ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualizationTarget {
    Gantt,
    EnergyDiagram,
}

// ── Validation ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}': {}", self.field, self.message)
    }
}

// ── Parsed output ─────────────────────────────────────────────────────────────

pub struct ParsedFileData {
    pub resources: Vec<Strata>,
    pub clusters: Vec<Cluster>,
    pub jobs: Vec<Job>,
    pub strata_by_resource_id: HashMap<u32, Strata>,
    /// Pre-computed energy series (timestamp_s, watts). When set, the energy
    /// diagram uses this directly instead of estimating from Job data.
    pub raw_energy_series: Option<Vec<(i64, f64)>>,
    /// Point-in-time annotations rendered generically on Gantt resource rows.
    pub markers: Vec<GanttMarker>,
}

// ── Core trait ────────────────────────────────────────────────────────────────

/// Implement this trait to add support for a new file type.
/// Register the implementation via `FileTypeRegistry::register()`.
/// Core import logic is untouched — only the registry and this trait are consulted.
pub trait FileTypeConfig: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn visualization_targets(&self) -> &[VisualizationTarget];
    /// Returns confidence in [0.0, 1.0] that this type matches `content`.
    fn detect(&self, content: &str) -> f32;
    fn validate(&self, content: &str) -> Vec<ValidationError>;
    fn parse(&self, content: &str) -> Result<ParsedFileData, String>;
    /// Whether the Gantt hierarchy/view controls (level selector, filters) are meaningful
    /// for this file type. Defaults to true; set false for flat annotation-only types.
    fn supports_hierarchy_controls(&self) -> bool { true }
    /// Aggregation levels to force when this file type is active.
    /// None  = use the live-data configured view (OAR-style).
    /// Some  = override options.levels with these field names.
    fn hierarchy_levels(&self) -> Option<Vec<String>> { None }
}

// ── Registry ──────────────────────────────────────────────────────────────────

pub struct FileTypeRegistry {
    types: Vec<Box<dyn FileTypeConfig>>,
}

impl FileTypeRegistry {
    pub fn new() -> Self {
        Self { types: Vec::new() }
    }

    /// Register a new file type. Call order determines tie-break priority.
    pub fn register(&mut self, file_type: Box<dyn FileTypeConfig>) {
        self.types.push(file_type);
    }

    /// Return the best-matching file type (confidence > 0.5), or None.
    pub fn detect<'a>(&'a self, content: &str) -> Option<&'a dyn FileTypeConfig> {
        self.types
            .iter()
            .map(|t| (t.as_ref(), t.detect(content)))
            .filter(|(_, c)| *c > 0.5)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(t, _)| t)
    }

    pub fn all_types(&self) -> impl Iterator<Item = &dyn FileTypeConfig> {
        self.types.iter().map(|t| t.as_ref())
    }

    pub fn find_by_name(&self, name: &str) -> Option<&dyn FileTypeConfig> {
        self.types.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }
}

impl Default for FileTypeRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(oar::OarFileType::new()));
        registry.register(Box::new(energy_series::EnergySeriesFileType::new()));
        registry.register(Box::new(event::EventFileType::new()));
        registry
    }
}

use super::{cluster::Cluster, job::Job, strata::Strata};
use crate::models::file_types::VisualizationTarget;

#[derive(Clone, Debug)]
pub struct ImportedDataSource {
    pub name: String,
    pub file_path: Option<String>,
    pub file_type_name: String,
    pub visualization_targets: Vec<VisualizationTarget>,
    pub jobs: Vec<Job>,
    pub clusters: Vec<Cluster>,
    pub strata: Vec<Strata>,
    /// Pre-computed energy series (timestamp_s, watts) from file types that provide raw power data.
    pub raw_energy_series: Option<Vec<(i64, f64)>>,
}

/// File selected but not yet imported — awaiting user type selection.
pub struct PendingImport {
    pub content: String,
    pub path: Option<String>,
    /// None = auto-detect; Some(name) = specific registered type.
    pub selected_type_name: Option<String>,
}

/// Tracks all file-import tabs: the list of loaded sources and which one is active.
/// Index 0 is always "Live Data" (not stored here); index 1+ maps to data_sources[index-1].
pub struct ImportState {
    pub imported_data_sources: Vec<ImportedDataSource>,
    pub current_data_source_index: usize,
    pub request_file_import: bool,
    pub pending_import: Option<PendingImport>,
}

impl Default for ImportState {
    fn default() -> Self {
        Self {
            imported_data_sources: Vec::new(),
            current_data_source_index: 0,
            request_file_import: false,
            pending_import: None,
        }
    }
}

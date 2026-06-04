use serde::{Deserialize, Serialize};

use super::gantt_config::GanttConfig;
use crate::views::components::gantt_job_color::JobColor;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClusterPreset {
    pub name: String,
    pub clusters: Vec<String>,
}

/// User-facing preferences and Gantt display state.
pub struct UiPreferences {
    pub font_size: i32,
    pub see_all_jobs: bool,
    pub theme_toggle_requested: bool,
    pub gantt_config: GanttConfig,
    pub cluster_presets: Vec<ClusterPreset>,
    pub job_color: JobColor,
    /// Set to true when Apply is clicked in ConfigPanel; GanttChart syncs options once then clears.
    pub config_reload_requested: bool,

    /// Updated each frame by GanttChart so the toolbar can show view-specific stats.
    pub current_gantt_view_name: String,
    pub current_gantt_view_levels: Vec<String>,
    pub current_gantt_view_summary_fields: Vec<String>,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            font_size: 16,
            see_all_jobs: false,
            theme_toggle_requested: false,
            gantt_config: GanttConfig::load(),
            cluster_presets: Vec::new(),
            job_color: JobColor::default(),
            config_reload_requested: false,
            current_gantt_view_name: String::new(),
            current_gantt_view_levels: Vec::new(),
            current_gantt_view_summary_fields: Vec::new(),
        }
    }
}

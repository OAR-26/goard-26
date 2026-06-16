use super::filters::JobFilters;
use super::job::Job;
use super::cluster::Cluster;
use super::job_data::JobData;
use super::ui_preferences::UiPreferences;
use crate::models::data_structure::job_sorting::JobSortable;
use crate::models::data_structure::view_type::ViewType;
use chrono::{DateTime, Local};

pub struct ApplicationContext {
    pub data: JobData,
    pub prefs: UiPreferences,

    // Session state
    pub view_type: ViewType,
    pub is_loading: bool,
    pub filters: JobFilters,
    /// Whether the current view represents a live snapshot (vs. imported/static
    /// data). Core uses this only to decide rendering choices (e.g. synthesizing
    /// the "all resources" virtual job); it has no idea how live data is fetched.
    pub live_data: bool,

    pub start_date: DateTime<Local>,
    pub end_date: DateTime<Local>,

    // Status flags — set by the binary each frame, read by core for display
    // (spinner, disabled refresh button). Core has no refresh engine of its own.
    pub is_refreshing: bool,
    pub live_refresh_paused: bool,
    /// Refresh-rate preference set via Tools' dropdown; the binary applies it
    /// to its own refresh engine.
    pub desired_refresh_rate_s: u64,

    // Signals set by views; consumed and acted on by the binary-level App.
    pub refresh_requested: bool,

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
    pub fn get_start_date(&self) -> DateTime<Local> {
        self.start_date
    }

    pub fn get_end_date(&self) -> DateTime<Local> {
        self.end_date
    }

    pub fn set_localdate(&mut self, start: DateTime<Local>, end: DateTime<Local>) {
        self.start_date = start;
        self.end_date = end;
    }

    /// Generic per-frame refresh: re-applies the time window to filters and
    /// re-filters jobs. Any binary can call this after updating `self.data`,
    /// regardless of where that data came from.
    pub fn refresh_filters(&mut self) {
        self.filters.set_scheduled_start_time(self.start_date.timestamp());
        self.filters.set_wall_time(self.end_date.timestamp());
        self.filter_jobs();
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
        let selected_cluster_names = self.filters.selected_cluster_names.clone();

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
            data: JobData::default(),
            prefs: UiPreferences::default(),

            view_type: ViewType::Gantt,
            is_loading: false,
            filters: JobFilters::default(),
            live_data: false,

            start_date: now,
            end_date: now,
            is_refreshing: false,
            live_refresh_paused: false,
            desired_refresh_rate_s: 30,
            refresh_requested: false,
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
        // Center the initial live-data window on now using default_timespan so
        // the "now" line appears at the center of the Gantt on first load.
        let half = chrono::Duration::seconds(context.prefs.gantt_config.default_timespan_s / 2);
        context.start_date = now - half;
        context.end_date   = now + half;
        context
    }
}

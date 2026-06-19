use super::filters::JobFilters;
use super::job::Job;
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

    pub start_date: DateTime<Local>,
    pub end_date: DateTime<Local>,

    // Flat display state — set by the binary. Core has no knowledge of tabs,
    // groups, files, or how sources are managed; it just renders these.
    pub show_energy: bool,
    pub show_gantt_panel: bool,

    /// One-shot signal: the binary just swapped the active data source.
    /// Core reads this once to refit its view to the new data, then clears it.
    /// Carries no identity — core doesn't track *which* source, just that it changed.
    pub source_changed: bool,

    /// Whether the Gantt's hierarchy-view controls (the "View:" dropdown)
    /// should be shown/interactive for the current source.
    pub show_hierarchy_controls: bool,

    /// Whether to additionally show a job-based energy estimate alongside
    /// the raw energy series for the current source (e.g. a Gantt source
    /// combined with separate energy-file sources sharing one view).
    pub show_estimated_with_energy: bool,

    /// Whether to synthesize the "all resources" virtual job row in the Gantt
    /// each frame. File-imported OAR data already carries this row from the
    /// parser; a binary with no file backing its data (e.g. a live feed) sets
    /// this to true so core fabricates the row instead.
    pub show_all_resources_row: bool,
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

    pub fn show_energy_diagram(&self) -> bool {
        self.show_energy
    }

    pub fn show_gantt(&self) -> bool {
        self.show_gantt_panel
    }

    pub fn get_current_energy_series(&self) -> Option<&[(i64, f64)]> {
        self.data.energy_series.first().map(|(_, s)| s.as_slice())
    }

    pub fn get_current_energy_series_multi(&self) -> Vec<(&str, &[(i64, f64)])> {
        self.data.energy_series.iter()
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

            start_date: now,
            end_date: now,
            show_energy: true,
            show_gantt_panel: true,
            source_changed: false,
            show_hierarchy_controls: true,
            show_estimated_with_energy: false,
            show_all_resources_row: false,
        };
        // Center the initial window on now using default_timespan so
        // the "now" line appears at the center of the Gantt on first load.
        let half = chrono::Duration::seconds(context.prefs.gantt_config.default_timespan_s / 2);
        context.start_date = now - half;
        context.end_date   = now + half;
        context
    }
}

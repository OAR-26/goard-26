use super::job::JobState;

#[derive(Default, Debug, Clone)]

pub struct JobFilters {
    pub owners: Option<Vec<String>>,
    pub states: Option<Vec<JobState>>,
    pub scheduled_start_time: Option<i64>,
    pub wall_time: Option<i64>,
    /// Cluster names to restrict the view to. Set directly by the binary
    /// (e.g. resolved from an app-level preset) — core has no preset concept.
    pub selected_cluster_names: Option<Vec<String>>,
}

#[allow(dead_code)]
impl JobFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy(filter: &JobFilters) -> Self {
        JobFilters {
            owners: filter.owners.clone(),
            states: filter.states.clone(),
            scheduled_start_time: filter.scheduled_start_time,
            wall_time: filter.wall_time,
            selected_cluster_names: filter.selected_cluster_names.clone(),
        }
    }

    pub fn set_owners(&mut self, owners: Option<Vec<String>>) {
        self.owners = owners;
    }

    pub fn set_states(&mut self, states: Option<Vec<JobState>>) {
        self.states = states;
    }

    pub fn set_scheduled_start_time(&mut self, scheduled_start_time: i64) {
        self.scheduled_start_time = Some(scheduled_start_time);
    }

    pub fn set_wall_time(&mut self, wall_time: i64) {
        self.wall_time = Some(wall_time);
    }

    pub fn set_selected_cluster_names(&mut self, cluster_names: Option<Vec<String>>) {
        self.selected_cluster_names = cluster_names;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

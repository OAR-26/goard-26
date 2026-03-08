use super::job::JobState;

#[derive(Default, Debug, Clone)]

pub struct JobFilters {
    pub owners: Option<Vec<String>>,
    pub states: Option<Vec<JobState>>,
    pub scheduled_start_time: Option<i64>,
    pub wall_time: Option<i64>,
    pub selected_preset: Option<String>,
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
            selected_preset: filter.selected_preset.clone(),
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

    pub fn set_selected_preset(&mut self, preset_name: Option<String>) {
        self.selected_preset = preset_name;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

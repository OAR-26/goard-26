use std::collections::HashMap;

use ganttza::models::data_structure::{job::Job, resource::DeadInterval, strata::Strata};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ApiSnapshot {
    pub jobs: Vec<Job>,
    pub resources: Vec<Strata>,
    pub dead_intervals: HashMap<u32, Vec<DeadInterval>>,
}

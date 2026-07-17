use goard_core::models::data_structure::{job::Job, strata::Strata};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ApiSnapshot {
    pub jobs: Vec<Job>,
    pub resources: Vec<Strata>,
}

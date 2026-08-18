use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Local};

use ganttza::models::data_structure::{job::Job, resource::DeadInterval, strata::Strata};

/// Owns every field that background refresh threads share with the UI thread:
/// the date window, refresh rate, refresh-in-progress flag, and all MPSC channels.
pub struct RefreshCoordinator {
    pub start_date: Arc<Mutex<DateTime<Local>>>,
    pub end_date: Arc<Mutex<DateTime<Local>>>,
    pub is_refreshing: Arc<Mutex<bool>>,
    pub refresh_rate: Arc<Mutex<u64>>,
    pub thread_started: bool,
    /// Incremented on every new instant_update call. An in-flight WASM fetch
    /// checks its captured generation against this before applying results —
    /// if they differ, a newer request has superseded it and results are dropped.
    pub request_gen: Arc<Mutex<u64>>,

    pub jobs_sender: Sender<Vec<Job>>,
    pub jobs_receiver: Receiver<Vec<Job>>,
    pub resources_sender: Sender<Vec<Strata>>,
    pub resources_receiver: Receiver<Vec<Strata>>,
    pub dead_intervals_sender: Sender<HashMap<u32, Vec<DeadInterval>>>,
    pub dead_intervals_receiver: Receiver<HashMap<u32, Vec<DeadInterval>>>,
}

impl RefreshCoordinator {
    pub fn new(now: DateTime<Local>) -> Self {
        let (jobs_sender, jobs_receiver) = channel();
        let (resources_sender, resources_receiver) = channel();
        let (dead_intervals_sender, dead_intervals_receiver) = channel();
        Self {
            start_date: Arc::new(Mutex::new(now - chrono::Duration::hours(1))),
            end_date: Arc::new(Mutex::new(now + chrono::Duration::hours(1))),
            is_refreshing: Arc::new(Mutex::new(false)),
            refresh_rate: Arc::new(Mutex::new(30)),
            thread_started: false,
            request_gen: Arc::new(Mutex::new(0)),
            jobs_sender,
            jobs_receiver,
            resources_sender,
            resources_receiver,
            dead_intervals_sender,
            dead_intervals_receiver,
        }
    }
}

use chrono::{DateTime, Local};

use std::time::Duration;

use crate::models::data_structure::application_context::ApplicationContext;

#[cfg(target_arch = "wasm32")]
use super::mocker::{mock_jobs,mock_stratas};


#[cfg(not(target_arch = "wasm32"))]
use crate::models::utils::parser::get_current_jobs_for_period;

#[cfg(not(target_arch = "wasm32"))]
use std::thread;

use super::parser::{get_dead_intervals_from_json, get_jobs_from_json, get_resources_from_json};

impl ApplicationContext {
    pub fn update_refresh_rate(&mut self, new_rate: u64) {
        let mut rate = self.refresh.refresh_rate.lock().unwrap();
        *rate = new_rate;
    }

    #[allow(dead_code)]
    pub fn update_start_date(&mut self, new_start: DateTime<Local>) {
        let mut start = self.refresh.start_date.lock().unwrap();
        *start = new_start;
    }

    #[allow(dead_code)]
    pub fn update_end_date(&mut self, new_end: DateTime<Local>) {
        let mut end = self.refresh.end_date.lock().unwrap();
        *end = new_end;
    }

    #[allow(dead_code)]
    pub fn get_start_date(&self) -> DateTime<Local> {
        *self.refresh.start_date.lock().unwrap()
    }

    #[allow(dead_code)]
    pub fn get_end_date(&self) -> DateTime<Local> {
        *self.refresh.end_date.lock().unwrap()
    }

    pub fn set_localdate(&mut self, start: DateTime<Local>, end: DateTime<Local>) {
        let mut start_date = self.refresh.start_date.lock().unwrap();
        let mut end_date = self.refresh.end_date.lock().unwrap();
        *start_date = start;
        *end_date = end;
    }

    pub fn instant_update(&mut self) {
        let is_refreshing = self.refresh.is_refreshing.clone();

        if *is_refreshing.lock().unwrap() {
            return;
        }
        *is_refreshing.lock().unwrap() = true;

        let start = *self.refresh.start_date.lock().unwrap();
        let end = *self.refresh.end_date.lock().unwrap();

        let jobs_sender = self.refresh.jobs_sender.clone();
        let resources_sender = self.refresh.resources_sender.clone();
        let dead_intervals_sender = self.refresh.dead_intervals_sender.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let is_refreshing_clone = is_refreshing.clone();
            thread::spawn(move || {
                let res = get_current_jobs_for_period(start, end);
                if res {
                    let jobs = get_jobs_from_json("./data/data.json");
                    let resources = get_resources_from_json("./data/data.json");
                    let dead_intervals = get_dead_intervals_from_json("./data/data.json");
                    jobs_sender.send(jobs).unwrap_or_else(|e| {
                        println!("Error while sending jobs: {}", e);
                    });
                    resources_sender.send(resources).unwrap_or_else(|e| {
                        println!("Error while sending resources: {}", e);
                    });
                    dead_intervals_sender.send(dead_intervals).unwrap_or_else(|e| {
                        println!("Error while sending dead intervals: {}", e);
                    });
                }
                *is_refreshing_clone.lock().unwrap() = false;
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let jobs = mock_jobs();
            jobs_sender.send(jobs).unwrap();
            let strata = mock_stratas();
            resources_sender.send(strata).unwrap();
            *is_refreshing.lock().unwrap() = false;
        }
    }

    pub fn update_periodically(&mut self) {
        if self.refresh.thread_started {
            // Thread already running — just unpause it.
            *self.refresh.refresh_rate.lock().unwrap() = 30;
            return;
        }
        self.refresh.thread_started = true;
        let refresh_rate = self.refresh.refresh_rate.clone();
        let jobs_sender = self.refresh.jobs_sender.clone();
        let resources_sender = self.refresh.resources_sender.clone();
        let dead_intervals_sender = self.refresh.dead_intervals_sender.clone();
        let is_refreshing = self.refresh.is_refreshing.clone();
        let start_date = self.refresh.start_date.clone();
        let end_date = self.refresh.end_date.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            thread::spawn(move || {
                loop {
                    let rate = *refresh_rate.lock().unwrap();

                    if rate == u64::MAX {
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }

                    if *is_refreshing.lock().unwrap() {
                        thread::sleep(Duration::from_secs(rate));
                        continue;
                    }

                    *is_refreshing.lock().unwrap() = true;

                    let start;
                    let end;
                    {
                        start = *start_date.lock().unwrap();
                        end = *end_date.lock().unwrap();
                    }

                    let res = get_current_jobs_for_period(start, end);
                    if res {
                        let jobs = get_jobs_from_json("./data/data.json");
                        let resources = get_resources_from_json("./data/data.json");
                        let dead_intervals = get_dead_intervals_from_json("./data/data.json");

                        jobs_sender.send(jobs).unwrap_or_else(|e| {
                            println!("Error while sending jobs: {}", e);
                        });
                        resources_sender.send(resources).unwrap_or_else(|e| {
                            println!("Error while sending resources: {}", e);
                        });
                        dead_intervals_sender.send(dead_intervals).unwrap_or_else(|e| {
                            println!("Error while sending dead intervals: {}", e);
                        });
                    }

                    *is_refreshing.lock().unwrap() = false;
                    thread::sleep(Duration::from_secs(rate));
                }
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let jobs = mock_jobs();
            jobs_sender.send(jobs).unwrap();
            let strata = mock_stratas();
            resources_sender.send(strata).unwrap();
        }
    }

    #[allow(dead_code)]
    pub fn update_period(&mut self, start_date: DateTime<Local>, end_date: DateTime<Local>) {
        self.update_start_date(start_date);
        self.update_end_date(end_date);
        self.is_loading = true;

        let sender = self.refresh.jobs_sender.clone();
        let start = start_date;
        let end = end_date;

        #[cfg(not(target_arch = "wasm32"))]
        {
            thread::spawn(move || {
                let res = get_current_jobs_for_period(start, end);
                if res {
                    let jobs = get_jobs_from_json("./data/data.json");
                    sender.send(jobs).unwrap();
                }
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let jobs = mock_jobs();
            sender.send(jobs).unwrap();
        }
    }
}

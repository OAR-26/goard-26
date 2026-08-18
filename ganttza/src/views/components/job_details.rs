use crate::models::data_structure::job::Job;
use crate::models::data_structure::strata::Strata;
use crate::models::utils::date_converter::format_timestamp;
use eframe::egui;
use std::collections::HashMap;

pub struct JobDetailsWindow {
    pub open: bool,
    pub job: Job,
    /// strata for this job's assigned resources: (cluster_name, host_name, strata)
    pub job_strata: Vec<(Option<String>, Option<String>, Strata)>,
}

impl JobDetailsWindow {
    pub fn new(job: Job, strata_by_resource_id: &HashMap<u32, Strata>) -> Self {
        let mut job_strata: Vec<(Option<String>, Option<String>, Strata)> = job
            .assigned_resources
            .iter()
            .filter_map(|rid| {
                strata_by_resource_id
                    .get(rid)
                    .map(|s| (s.cluster.clone(), s.host.clone(), s.clone()))
            })
            .collect();
        job_strata.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        Self { open: true, job, job_strata }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // If the window is not open, do not render it
        if !self.open {
            return;
        }

        egui::Window::new(format!(
            "{}: {}",
            t!("app.details.general.title"),
            self.job.id
        ))
        .collapsible(true)
        .movable(true)
        .open(&mut self.open)
        .vscroll(true)
        .show(ui.ctx(), |ui| {
            // Base information
            ui.group(|ui| {
                ui.heading(t!("app.details.basic_info.title"));
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", t!("app.details.basic_info.owner")));
                    ui.strong(self.job.owner.to_string());
                });
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", t!("app.details.basic_info.queue")));
                    ui.strong(self.job.queue.to_string());
                });
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", t!("app.details.basic_info.command")));
                    ui.strong(self.job.command.to_string());
                });
            });

            ui.add_space(8.0);

            // Status
            ui.group(|ui| {
                ui.heading(t!("app.details.status.title"));
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", t!("app.details.status.state")));
                    let state_text = self.job.state.get_label();
                    let (state_color, bg_color) = self.job.state.get_color();
                    ui.label(
                        egui::RichText::new(state_text)
                            .color(state_color)
                            .background_color(bg_color)
                            .strong(),
                    );
                });
                if let Some(exit_code) = self.job.exit_code {
                    ui.horizontal(|ui| {
                        ui.label("Exit Code: ");
                        ui.strong(exit_code.to_string());
                    });
                }
                if let Some(message) = &self.job.message {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", t!("app.details.status.message")));
                        ui.label(message);
                    });
                }
            });

            ui.add_space(8.0);

            // Timing information
            ui.group(|ui| {
                ui.heading(t!("app.details.timing_info.title"));

                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}:",
                        t!("app.details.timing_info.submission_time")
                    ));
                    ui.strong(format_timestamp(self.job.submission_time));
                });
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}:",
                        t!("app.details.timing_info.scheduled_start_time")
                    ));
                    ui.strong(format_timestamp(self.job.scheduled_start));
                });
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}:",
                        t!("app.details.timing_info.actual_start_time")
                    ));
                    ui.strong(format_timestamp(self.job.start_time));
                });
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", t!("app.details.timing_info.stop_time")));
                    ui.strong(format_timestamp(self.job.stop_time));
                });
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", t!("app.details.timing_info.wall_time")));
                    ui.strong(format!("{} seconds", self.job.walltime));
                });
            });

            ui.add_space(8.0);

            if !self.job_strata.is_empty() {
                // Resources — grouped by cluster → host
                ui.group(|ui| {
                    ui.heading(t!("app.details.resources.title"));
                    egui::CollapsingHeader::new(t!("app.details.resources.cluster"))
                        .default_open(false)
                        .show(ui, |ui| {
                            // Group by cluster
                            let mut by_cluster: std::collections::BTreeMap<
                                String,
                                std::collections::BTreeMap<String, Vec<&Strata>>,
                            > = std::collections::BTreeMap::new();
                            for (cluster_opt, host_opt, strata) in &self.job_strata {
                                let cluster =
                                    cluster_opt.clone().unwrap_or_else(|| "(no cluster)".to_string());
                                let host =
                                    host_opt.clone().unwrap_or_else(|| "(no host)".to_string());
                                by_cluster
                                    .entry(cluster)
                                    .or_default()
                                    .entry(host)
                                    .or_default()
                                    .push(strata);
                            }

                            for (cluster_name, hosts) in &by_cluster {
                                egui::CollapsingHeader::new(cluster_name)
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        for (host_name, strata_list) in hosts {
                                            egui::CollapsingHeader::new(host_name)
                                                .default_open(false)
                                                .show(ui, |ui| {
                                                    // Show host-level fields from the first strata entry
                                                    if let Some(s) = strata_list.first() {
                                                        if let Some(addr) = &s.network_address {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Network Address: ");
                                                                ui.strong(addr);
                                                            });
                                                        }
                                                        if let Some(chassis) = &s.chassis {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Chassis: ");
                                                                ui.strong(chassis);
                                                            });
                                                        }
                                                        if let Some(cpufreq) = &s.cpufreq {
                                                            ui.horizontal(|ui| {
                                                                ui.label("CPU Frequency: ");
                                                                ui.strong(cpufreq);
                                                            });
                                                        }
                                                        if let Some(cputype) = &s.cputype {
                                                            ui.horizontal(|ui| {
                                                                ui.label("CPU Type: ");
                                                                ui.strong(cputype);
                                                            });
                                                        }
                                                        if let Some(core_count) = s.core_count {
                                                            ui.horizontal(|ui| {
                                                                ui.label("Core Count: ");
                                                                ui.strong(core_count.to_string());
                                                            });
                                                        }
                                                    }

                                                    // Show each resource (strata entry)
                                                    for strata in strata_list {
                                                        let rid_str = strata
                                                            .resource_id
                                                            .map(|id| id.to_string())
                                                            .unwrap_or_else(|| "?".to_string());
                                                        egui::CollapsingHeader::new(
                                                            format!("Resource {}", rid_str),
                                                        )
                                                        .default_open(false)
                                                        .show(ui, |ui| {
                                                            if let Some(state) = &strata.state {
                                                                ui.horizontal(|ui| {
                                                                    ui.label("State: ");
                                                                    ui.strong(state);
                                                                });
                                                            }
                                                            if let Some(tc) = strata.thread_count {
                                                                ui.horizontal(|ui| {
                                                                    ui.label("Thread Count: ");
                                                                    ui.strong(tc.to_string());
                                                                });
                                                            }
                                                        });
                                                    }
                                                });
                                        }
                                    });
                            }
                        });
                });
            }

            ui.add_space(8.0);
        });
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}

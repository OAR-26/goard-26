use goard_core::models::data_structure::job::Job;

/// Estimates global power (W) over [start_s, end_s] from job allocations.
pub fn estimate_from_jobs(
    jobs: &[Job],
    start_s: i64,
    end_s: i64,
    step_s: i64,
    watts_per_resource: f64,
) -> Vec<(i64, f64)> {
    if end_s <= start_s || step_s <= 0 {
        return Vec::new();
    }
    let relevant: Vec<&Job> = jobs.iter().filter(|j| {
        let je = j.scheduled_start + j.walltime as i64;
        je >= start_s && j.scheduled_start <= end_s
    }).collect();

    let mut out = Vec::new();
    let mut t = start_s;
    while t <= end_s {
        let units: usize = relevant.iter().filter_map(|j| {
            let js = j.scheduled_start;
            let je = js + j.walltime as i64;
            if js <= t && t <= je {
                Some(if !j.assigned_resources.is_empty() {
                    j.assigned_resources.len()
                } else {
                    j.hosts.len()
                })
            } else { None }
        }).sum();
        out.push((t, units as f64 * watts_per_resource));
        t += step_s;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use goard_core::models::data_structure::job::{Job, JobState};
    use goard_core::models::data_structure::resource::ResourceState;

    fn make_job(id: u32, start: i64, walltime: i64, resources: Vec<u32>) -> Job {
        Job {
            id,
            owner: String::new(),
            state: JobState::Running,
            command: String::new(),
            walltime,
            message: None,
            queue: String::new(),
            assigned_resources: resources,
            scheduled_start: start,
            submission_time: 0,
            start_time: start,
            stop_time: start + walltime,
            exit_code: None,
            clusters: Vec::new(),
            hosts: Vec::new(),
            main_resource_state: ResourceState::Alive,
            job_type: String::new(),
            job_types: Vec::new(),
            name: None,
            project: String::new(),
        }
    }

    #[test]
    fn estimate_no_jobs_gives_zeros() {
        let pts = estimate_from_jobs(&[], 0, 100, 10, 300.0);
        assert_eq!(pts.len(), 11);
        assert!(pts.iter().all(|(_, w)| *w == 0.0));
    }

    #[test]
    fn estimate_single_job_correct_watts() {
        let job = make_job(1, 0, 100, vec![10, 11]);
        let pts = estimate_from_jobs(&[job], 0, 100, 10, 300.0);
        assert!(pts.iter().all(|(_, w)| (*w - 600.0).abs() < 1e-9));
    }

    #[test]
    fn estimate_job_outside_window_ignored() {
        let job = make_job(1, 200, 100, vec![1, 2, 3]);
        let pts = estimate_from_jobs(&[job], 0, 100, 10, 300.0);
        assert!(pts.iter().all(|(_, w)| *w == 0.0));
    }

    #[test]
    fn estimate_partial_overlap() {
        let job = make_job(1, 50, 150, vec![5]);
        let pts = estimate_from_jobs(&[job], 0, 100, 50, 100.0);
        assert_eq!(pts, vec![(0, 0.0), (50, 100.0), (100, 100.0)]);
    }

    #[test]
    fn estimate_returns_empty_for_invalid_range() {
        assert!(estimate_from_jobs(&[], 100, 50, 10, 300.0).is_empty());
        assert!(estimate_from_jobs(&[], 0, 100, 0, 300.0).is_empty());
    }
}

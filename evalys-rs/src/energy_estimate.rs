use ganttza::models::data_structure::job::Job;

/// Builds XY series points from a raw measured series, adding zero-padding
/// at the edges so panning outside the data shows zero.
pub fn series_from_raw(raw: &[(i64, f64)]) -> Vec<(i64, f64)> {
    if raw.is_empty() { return Vec::new(); }
    const FAR: i64 = 10_000_000_000; // ~317 years
    let mut points = Vec::with_capacity(raw.len() + 4);
    let first_ts = raw[0].0;
    let last_ts = raw[raw.len() - 1].0;
    points.push((first_ts - FAR, 0.0));
    points.push((first_ts - 1, 0.0));
    points.extend_from_slice(raw);
    points.push((last_ts + 1, 0.0));
    points.push((last_ts + FAR, 0.0));
    points
}

/// Estimates global power (W) over [start_s, end_s] from job allocations.
/// `step_s`: time resolution in seconds.
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
    use ganttza::models::data_structure::job::{Job, JobState};
    use ganttza::models::data_structure::resource::ResourceState;

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
        assert_eq!(pts.len(), 11); // 0, 10, 20, ..., 100
        assert!(pts.iter().all(|(_, w)| *w == 0.0));
    }

    #[test]
    fn estimate_single_job_correct_watts() {
        // job holds 2 resources for t=0..100, 300W each
        let job = make_job(1, 0, 100, vec![10, 11]);
        let pts = estimate_from_jobs(&[job], 0, 100, 10, 300.0);
        // every sample should be 2 * 300 = 600W
        assert!(pts.iter().all(|(_, w)| (*w - 600.0).abs() < 1e-9));
    }

    #[test]
    fn estimate_job_outside_window_ignored() {
        // job runs 200..300, window is 0..100
        let job = make_job(1, 200, 100, vec![1, 2, 3]);
        let pts = estimate_from_jobs(&[job], 0, 100, 10, 300.0);
        assert!(pts.iter().all(|(_, w)| *w == 0.0));
    }

    #[test]
    fn estimate_partial_overlap() {
        // job runs t=50..200, window 0..100, step 50
        let job = make_job(1, 50, 150, vec![5]);
        let pts = estimate_from_jobs(&[job], 0, 100, 50, 100.0);
        // t=0: job not started → 0W; t=50: job active → 100W; t=100: active → 100W
        assert_eq!(pts, vec![(0, 0.0), (50, 100.0), (100, 100.0)]);
    }

    #[test]
    fn estimate_returns_empty_for_invalid_range() {
        let pts = estimate_from_jobs(&[], 100, 50, 10, 300.0); // end < start
        assert!(pts.is_empty());
        let pts2 = estimate_from_jobs(&[], 0, 100, 0, 300.0); // step = 0
        assert!(pts2.is_empty());
    }

    #[test]
    fn series_from_raw_empty() {
        assert!(series_from_raw(&[]).is_empty());
    }

    #[test]
    fn series_from_raw_pads_zeros() {
        let raw = vec![(1000i64, 500.0), (2000i64, 800.0)];
        let pts = series_from_raw(&raw);
        // first two points are zeros before data, last two are zeros after
        assert_eq!(pts[0].1, 0.0);
        assert_eq!(pts[1].1, 0.0);
        assert_eq!(pts[2], (1000, 500.0));
        assert_eq!(pts[3], (2000, 800.0));
        assert_eq!(pts[4].1, 0.0);
        assert_eq!(pts[5].1, 0.0);
        assert_eq!(pts.len(), 6);
    }
}

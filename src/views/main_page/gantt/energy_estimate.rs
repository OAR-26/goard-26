use crate::models::data_structure::job::Job;
/// Estimation de la puissance globale (W) sur une fenêtre [start_s, end_s].
///
/// Idée:
/// - Si `assigned_resources` est rempli => unités = assigned_resources.len()
/// - Sinon => unités = hosts.len() 
/// - Puissance = unités * watts_per_unit
///
/// `step_s` = pas en secondes (ex: 10 => 1 point toutes les 10s)
pub fn estimate_global_energy_series(
    jobs: &[Job],
    start_s: i64,
    end_s: i64,
    step_s: i64,
    watts_per_unit: f64,
) -> Vec<(i64, f64)> {
    if end_s <= start_s || step_s <= 0 {
        return Vec::new();
    }

    // Garder seulement les jobs qui intersectent la fenêtre (perf)
    let mut relevant: Vec<&Job> = Vec::new();
    for j in jobs {
        let js = j.scheduled_start;
        let je = j.scheduled_start + j.walltime as i64;
        if je >= start_s && js <= end_s {
            relevant.push(j);
        }
    }

    let mut out = Vec::new();
    let mut t = start_s;

    while t <= end_s {
        let mut total_units = 0usize;

        for j in &relevant {
            let js = j.scheduled_start;
            let je = j.scheduled_start + j.walltime as i64;

            if js <= t && t <= je {
                let units = if !j.assigned_resources.is_empty() {
                    j.assigned_resources.len()
                } else if !j.hosts.is_empty() {
                    j.hosts.len()
                } else {
                    0
                };

                total_units += units;
            }
        }

        let w = total_units as f64 * watts_per_unit;
        out.push((t, w));
        t += step_s;
    }

    out
}
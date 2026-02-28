use chrono::{Local, TimeZone};
use eframe::egui;
use egui_plot::{Line, Plot, PlotBounds, PlotPoint, PlotPoints, Text, VLine};

fn fmt_hhmm(ts: i64) -> String {
    Local.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn fmt_hhmmss(ts: i64) -> String {
    Local.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Affiche le graphe énergie.
/// Retourne Some((new_start_s, new_end_s)) si l'utilisateur a modifié la vue (drag/zoom).
pub fn ui_energy_global(
    ui: &mut egui::Ui,
    points_w: &[(i64, f64)],
    visible_start_s: i64,
    visible_end_s: i64,
    now_s: i64,
) -> Option<(i64, i64)> {
    ui.label("Consommation globale (estimée)");

    if points_w.is_empty() {
        ui.weak("Pas de données énergie pour cette fenêtre.");
        return None;
    }

    // Convertir en points plot + min/max global
    let mut global_y_min = f64::INFINITY;
    let mut global_y_max = f64::NEG_INFINITY;

    let pts: PlotPoints = points_w
        .iter()
        .map(|(t, w)| {
            global_y_min = global_y_min.min(*w);
            global_y_max = global_y_max.max(*w);
            [*t as f64, *w]
        })
        .collect();

    if !global_y_min.is_finite() || !global_y_max.is_finite() {
        ui.weak("Données énergie invalides.");
        return None;
    }

    let line = Line::new(pts).name("W");
    let now_line = VLine::new(now_s as f64).name("now");

    // Bounds initiales = fenêtre du Gantt + Y global (juste pour démarrer)
    let initial_bounds = PlotBounds::from_min_max(
        [visible_start_s as f64, global_y_min],
        [visible_end_s as f64, global_y_max],
    );

    let plot_resp = Plot::new("energy_global_plot")
        .height(230.0)
        .show_axes([true, true])
        .show_grid(true)
        .allow_drag(true)
        .allow_zoom(true)
        .x_axis_formatter(|mark, _| {
            let ts = mark.value.round() as i64;
            fmt_hhmm(ts)
        })
        .show(ui, |plot_ui| {
            // IMPORTANT: au premier frame, on force les bounds initiales
            // (sinon egui_plot autoscale et on perd la synchro)
            plot_ui.set_plot_bounds(initial_bounds);

            // Bounds courants (après interaction / set_plot_bounds)
            let b = plot_ui.plot_bounds();
            let vx0 = b.min()[0] as i64;
            let vx1 = b.max()[0] as i64;

            // Rescale Y sur la fenêtre X visible (pour ne pas "perdre" la courbe)
            let mut y_min = f64::INFINITY;
            let mut y_max = f64::NEG_INFINITY;
            for (t, w) in points_w {
                if *t >= vx0 && *t <= vx1 {
                    y_min = y_min.min(*w);
                    y_max = y_max.max(*w);
                }
            }

            if y_min.is_finite() && y_max.is_finite() {
                let pad = ((y_max - y_min).abs() * 0.10).max(1.0);
                plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                    [b.min()[0], y_min - pad],
                    [b.max()[0], y_max + pad],
                ));
            }

            plot_ui.line(line);
            plot_ui.vline(now_line);

            // Tooltip lisible (heure)
            if let Some(pos) = plot_ui.pointer_coordinate() {
                let ts = pos.x.round() as i64;
                let label = format!("{}  |  {:.0} W", fmt_hhmmss(ts), pos.y);
                plot_ui.text(Text::new(PlotPoint::new(pos.x, pos.y), label));
            }
        });

    // Bounds visibles après rendu (via transform)
    let b = plot_resp.transform.bounds();
    let new_start = b.min()[0].round() as i64;
    let new_end = b.max()[0].round() as i64;

    // Mini timeline sous le plot
    ui.horizontal(|ui| {
        ui.small(fmt_hhmm(new_start));
        ui.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| ui.small(fmt_hhmm((new_start + new_end) / 2)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.small(fmt_hhmm(new_end));
        });
    });

    // Détection interaction
    let scrolled = ui.input(|i| i.raw_scroll_delta.y != 0.0);
    if plot_resp.response.dragged()
        || plot_resp.response.double_clicked()
        || (plot_resp.response.hovered() && scrolled)
    {
        return Some((new_start, new_end));
    }

    None
}
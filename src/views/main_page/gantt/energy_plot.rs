use chrono::{Local, TimeZone};
use eframe::egui;
use egui::Vec2b;
use egui_plot::{
    CoordinatesFormatter, Corner, Line, Plot, PlotBounds, PlotPoints, VLine,
};

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

/// Affiche le graphe global de consommation d’énergie.
/// Le graphe est synchronisé avec la fenêtre temporelle visible du Gantt.
pub fn ui_energy_global(
    ui: &mut egui::Ui,
    points_w: &[(i64, f64)],
    visible_start_s: i64,
    visible_end_s: i64,
    now_s: i64,
    left_gutter_width_px: f32,
    height: f32,
    gantt_synced: bool,
    fit_to_figure: bool,
    y_bounds: Option<(f64, f64)>,
) -> (Option<(i64, i64)>, Option<(f64, f64)>) {
    ui.label("Consommation globale");

    if points_w.is_empty() {
        ui.weak("Pas de données énergie pour cette fenêtre.");
        return (None, None);
    }

    // Bornes globales
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
        return (None, None);
    }


    let line = Line::new(pts).color(egui::Color32::BLUE);
    let now_line = VLine::new(now_s as f64)
        .color(egui::Color32::RED)
        .width(2.0);

    // Texte affiché au survol, dessiné manuellement pour éviter le tooltip de egui_plot (x=..., y=...).
    let mut hover_label: Option<String> = None;

    // Capture BEFORE .show() — inside closure we may zero these to block egui_plot's X zoom.
    let alt_held = ui.input(|i| i.modifiers.alt);
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);

    let plot_resp = Plot::new("energy_global_plot")
        .height(height)
        .y_axis_min_width(left_gutter_width_px.max(0.0))
        .show_axes([true, true])
        .show_x(true)
        .show_y(true)
        .show_grid(true)
        .allow_drag(Vec2b::new(true, true))
        .allow_zoom(Vec2b::new(true, false))   // Y zoom via alt+scroll only
        .label_formatter(|_, _| String::new())
        .coordinates_formatter(
            Corner::LeftTop,
            CoordinatesFormatter::new(|_, _| String::new()),
        )
        .x_axis_formatter(|mark, _| {
            let ts = mark.value.round() as i64;
            fmt_hhmm(ts)
        })
        .show(ui, |plot_ui| {
            // egui_plot processes scroll for X zoom AFTER this closure returns.
            // Zero it here when alt is held + cursor is inside the plot so the plot
            // doesn't also X-zoom while we manually Y-zoom with alt+scroll.
            if alt_held && plot_ui.pointer_coordinate().is_some() {
                plot_ui.ctx().input_mut(|i| {
                    i.raw_scroll_delta = egui::Vec2::ZERO;
                    i.smooth_scroll_delta = egui::Vec2::ZERO;
                });
            }

            // X source: Gantt-synced → Gantt's viewport; standalone → last frame's plot bounds.
            let (vx0, vx1) = if gantt_synced {
                (visible_start_s as f64, visible_end_s as f64)
            } else {
                let cur = plot_ui.plot_bounds();
                (cur.min()[0], cur.max()[0])
            };

            // Compute Y that fits the currently visible X window.
            let mut y_min = f64::INFINITY;
            let mut y_max = f64::NEG_INFINITY;
            for (t, w) in points_w {
                if (*t as f64) >= vx0 && (*t as f64) <= vx1 {
                    y_min = y_min.min(*w);
                    y_max = y_max.max(*w);
                }
            }
            let y_fit = if y_min.is_finite() && y_max.is_finite() {
                let pad = ((y_max - y_min).abs() * 0.10).max(1.0);
                (y_min - pad, y_max + pad)
            } else {
                (global_y_min, global_y_max)
            };

            // Choose Y: fit mode → computed fit; free mode → stored (alt+scroll) or fit as fallback.
            let (y0, y1) = if fit_to_figure { y_fit } else { y_bounds.unwrap_or(y_fit) };

            // Unified bounds application — same logic for both modes; only X source differs.
            plot_ui.set_plot_bounds(PlotBounds::from_min_max([vx0, y0], [vx1, y1]));

            plot_ui.line(line);
            plot_ui.vline(now_line);
            // Tooltip personnalisé : heure exacte + puissance en watts
            if let Some(pos) = plot_ui.pointer_coordinate() {
                let ts = pos.x.round() as i64;
                hover_label = Some(format!("{}  |  {:.0} W", fmt_hhmmss(ts), pos.y));
            }
        });

        if let (Some(label), Some(mouse_pos)) = (hover_label, plot_resp.response.hover_pos()) {
            let painter = ui.painter();

            let font_id = egui::TextStyle::Body.resolve(ui.style());
            let text_color = egui::Color32::WHITE;
            let bg_color = egui::Color32::from_black_alpha(220);
            let padding = egui::vec2(6.0, 4.0);

            let galley = painter.layout_no_wrap(label, font_id, text_color);
            let rect = egui::Rect::from_min_size(
                mouse_pos + egui::vec2(12.0, 12.0),
                galley.size() + 2.0 * padding,
            );

            painter.rect_filled(rect, 4.0, bg_color);
            painter.galley(rect.min + padding, galley, text_color);
        }

        let b = plot_resp.transform.bounds();
        let new_start = b.min()[0].round() as i64;
        let new_end = b.max()[0].round() as i64;
        let mut new_y = (b.min()[1], b.max()[1]);

    // Alt+scroll: manual vertical zoom. Use pre-captured delta (may have been zeroed inside closure).
    if plot_resp.response.hovered() && alt_held && scroll_delta_y != 0.0 {
        if !fit_to_figure {
            let zoom = (1.0 + scroll_delta_y as f64 * 0.005).clamp(0.1, 10.0);
            let center = (new_y.0 + new_y.1) / 2.0;
            let half = (new_y.1 - new_y.0) / 2.0 / zoom;
            new_y = (center - half, center + half);
        }
        // Ensure consumed (closure may not have run if pointer was outside).
        ui.input_mut(|i| {
            i.raw_scroll_delta = egui::Vec2::ZERO;
            i.smooth_scroll_delta = egui::Vec2::ZERO;
        });
    }

    let scrolled = ui.input(|i| i.raw_scroll_delta.y != 0.0);
    if plot_resp.response.dragged()
        || plot_resp.response.double_clicked()
        || (plot_resp.response.hovered() && scrolled)
    {
        return (Some((new_start, new_end)), Some(new_y));
    }

    (None, Some(new_y))
}
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

fn series_color(idx: usize) -> egui::Color32 {
    const PALETTE: &[(u8, u8, u8)] = &[
        (31,  119, 180), // blue
        (255, 127,  14), // orange
        (44,  160,  44), // green
        (214,  39,  40), // red
        (148, 103, 189), // purple
        (140,  86,  75), // brown
        (227, 119, 194), // pink
        (188, 189,  34), // olive
    ];
    let (r, g, b) = PALETTE[idx % PALETTE.len()];
    egui::Color32::from_rgb(r, g, b)
}

/// Generic XY plot synchronized with the Gantt time window.
/// `series`: list of (label, points) — one entry per data source.
pub fn ui_xy_global(
    ui: &mut egui::Ui,
    series: &[(String, Vec<(i64, f64)>)],
    visible_start_s: i64,
    visible_end_s: i64,
    now_s: i64,
    left_gutter_width_px: f32,
    height: f32,
    gantt_synced: bool,
    fit_to_figure: bool,
    y_bounds: Option<(f64, f64)>,
    series_palette: &[[u8; 3]],
    now_line_color: egui::Color32,
    scroll_zoom_sensitivity: f32,
    zoom_max_s: f32,
    zoom_min_s: f32,
) -> (Option<(i64, i64)>, Option<(f64, f64)>) {
    let non_empty: Vec<&(String, Vec<(i64, f64)>)> = series.iter()
        .filter(|(_, pts)| !pts.is_empty())
        .collect();

    if non_empty.is_empty() {
        ui.weak("No data for this window.");
        return (None, None);
    }

    // Legend — only shown when there are 2+ series.
    if non_empty.len() > 1 {
        ui.horizontal(|ui| {
            for (idx, (label, _)) in non_empty.iter().enumerate() {
                let color = series_color(idx);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(14.0, 14.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 2.0, color);
                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120)));
                ui.label(label.as_str());
                if idx + 1 < non_empty.len() {
                    ui.separator();
                }
            }
        });
    }

    // Global Y bounds across all series.
    let mut global_y_min = f64::INFINITY;
    let mut global_y_max = f64::NEG_INFINITY;
    for (_, pts) in &non_empty {
        for (_, w) in pts.iter() {
            global_y_min = global_y_min.min(*w);
            global_y_max = global_y_max.max(*w);
        }
    }

    if !global_y_min.is_finite() || !global_y_max.is_finite() {
        ui.weak("Invalid data.");
        return (None, None);
    }

    // Build egui_plot Lines — one per series.
    let lines: Vec<Line> = non_empty.iter().enumerate().map(|(idx, (label, pts))| {
        let color = if non_empty.len() == 1 {
            if let Some(&[r, g, b]) = series_palette.first() {
                egui::Color32::from_rgb(r, g, b)
            } else {
                series_color(idx)
            }
        } else if let Some(&[r, g, b]) = series_palette.get(idx % series_palette.len().max(1)) {
            egui::Color32::from_rgb(r, g, b)
        } else {
            series_color(idx)
        };
        let plot_pts: PlotPoints = pts.iter().map(|(t, w)| [*t as f64, *w]).collect();
        let mut line = Line::new(plot_pts).color(color);
        if non_empty.len() > 1 {
            line = line.name(label.as_str());
        }
        line
    }).collect();

    let now_line = VLine::new(now_s as f64)
        .color(now_line_color)
        .width(2.0);

    let mut hover_label: Option<String> = None;

    // Capture BEFORE .show() — inside closure we consume these to block scroll bubbling.
    let alt_held = ui.input(|i| i.modifiers.alt);
    let ctrl_held = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);

    let plot_resp = Plot::new("xy_global_plot")
        .height(height)
        .y_axis_min_width(left_gutter_width_px.max(0.0))
        .show_axes([true, true])
        .show_x(true)
        .show_y(true)
        .show_grid(true)
        .allow_drag(Vec2b::new(true, true))
        .allow_zoom(Vec2b::new(false, false)) // X zoom handled manually below
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
            // Consume scroll when Alt or Ctrl held so it doesn't bubble to the parent.
            if (alt_held || ctrl_held) && plot_ui.pointer_coordinate().is_some() {
                plot_ui.ctx().input_mut(|i| {
                    i.raw_scroll_delta = egui::Vec2::ZERO;
                    i.smooth_scroll_delta = egui::Vec2::ZERO;
                });
            }

            let (vx0, vx1) = (visible_start_s as f64, visible_end_s as f64);

            // Y that fits visible X window across all series.
            let mut y_min = f64::INFINITY;
            let mut y_max = f64::NEG_INFINITY;
            for (_, pts) in &non_empty {
                for (t, w) in pts.iter() {
                    if (*t as f64) >= vx0 && (*t as f64) <= vx1 {
                        y_min = y_min.min(*w);
                        y_max = y_max.max(*w);
                    }
                }
            }
            let y_fit = if y_min.is_finite() && y_max.is_finite() {
                let pad = ((y_max - y_min).abs() * 0.10).max(1.0);
                (y_min - pad, y_max + pad)
            } else {
                (global_y_min, global_y_max)
            };

            let (y0, y1) = if fit_to_figure { y_fit } else { y_bounds.unwrap_or(y_fit) };
            plot_ui.set_plot_bounds(PlotBounds::from_min_max([vx0, y0], [vx1, y1]));

            for line in lines {
                plot_ui.line(line);
            }
            plot_ui.vline(now_line);

            if let Some(pos) = plot_ui.pointer_coordinate() {
                let ts = pos.x.round() as i64;
                hover_label = Some(format!("{}  |  {:.2}", fmt_hhmmss(ts), pos.y));
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
    let drag_start = b.min()[0].round() as i64;
    let drag_end = b.max()[0].round() as i64;
    let mut new_y = (b.min()[1], b.max()[1]);

    // Y zoom (Alt+Scroll) — unchanged.
    if plot_resp.response.hovered() && alt_held && scroll_delta_y != 0.0 {
        if !fit_to_figure {
            let zoom = (1.0 + scroll_delta_y as f64 * 0.005).clamp(0.1, 10.0);
            let center = (new_y.0 + new_y.1) / 2.0;
            let half = (new_y.1 - new_y.0) / 2.0 / zoom;
            new_y = (center - half, center + half);
        }
    }

    // X zoom (Ctrl+Scroll) — manual, same logic as Gantt interaction.rs, respects limits.
    if plot_resp.response.hovered() && ctrl_held && scroll_delta_y != 0.0 {
        let current_interval = (visible_end_s - visible_start_s).max(1) as f32;
        let zoom_factor = (scroll_delta_y * scroll_zoom_sensitivity).exp();
        let new_interval = current_interval / zoom_factor;
        if new_interval >= zoom_min_s && new_interval <= zoom_max_s {
            let center = (visible_start_s + visible_end_s) / 2;
            let half = (new_interval / 2.0) as i64;
            return (Some((center - half, center - half + new_interval as i64)), Some(new_y));
        }
        // Outside limits — return nothing, visible range stays unchanged.
        return (None, Some(new_y));
    }

    // X pan (drag or double-click reset from egui_plot).
    let scrolled = ui.input(|i| i.raw_scroll_delta.y != 0.0);
    if plot_resp.response.dragged()
        || plot_resp.response.double_clicked()
        || (plot_resp.response.hovered() && scrolled)
    {
        return (Some((drag_start, drag_end)), Some(new_y));
    }

    (None, Some(new_y))
}

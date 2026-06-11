use super::theme::get_theme_colors;
use super::types::{Info, Options};
use chrono::{DateTime, Local};
use egui::{pos2, remap_clamp, Align2, Color32, Rgba, Rect, Stroke};

#[derive(Clone, Copy)]
struct GridLevel {
    step_s: i64,
    medium_s: i64,
    big_s: i64,
}

// Sorted smallest→largest step_s.
// Labels rendered at medium_s (medium alpha) and big_s (big alpha).
// pick_grid_level selects the finest level where canvas_width_s / step_s <= max_lines.
const GRID_LEVELS: &[GridLevel] = &[
    GridLevel { step_s: 1,      medium_s: 1,       big_s: 5       }, // 1s / 5s
    GridLevel { step_s: 1,      medium_s: 5,       big_s: 30      }, // 5s / 30s
    GridLevel { step_s: 2,      medium_s: 10,      big_s: 60      }, // 10s / 1min
    GridLevel { step_s: 3,      medium_s: 30,      big_s: 60      }, // 30s / 1min
    GridLevel { step_s: 6,      medium_s: 60,      big_s: 300     }, // 1min / 5min
    GridLevel { step_s: 30,     medium_s: 300,     big_s: 600     }, // 5min / 10min
    GridLevel { step_s: 90,     medium_s: 900,     big_s: 1800    }, // 15min / 30min
    GridLevel { step_s: 180,    medium_s: 1800,    big_s: 3600    }, // 30min / 1h
    GridLevel { step_s: 360,    medium_s: 3600,    big_s: 7200    }, // 1h / 2h
    GridLevel { step_s: 720,    medium_s: 7200,    big_s: 14400   }, // 2h / 4h
    GridLevel { step_s: 1800,   medium_s: 18000,   big_s: 36000   }, // 5h / 10h
    GridLevel { step_s: 7200,   medium_s: 72000,   big_s: 144000  }, // 20h / 40h
    GridLevel { step_s: 86400,  medium_s: 864000,  big_s: 1728000 }, // 10d / 20d
    GridLevel { step_s: 864000, medium_s: 8640000, big_s: 17280000}, // 100d / 200d
];

fn pick_grid_level(canvas_width_s: f32, max_lines: f32) -> GridLevel {
    // Target ~max_lines/10 medium labels on screen → ~130px spacing for any level.
    let max_labels = max_lines / 10.0;
    for &lvl in GRID_LEVELS {
        if canvas_width_s / (lvl.medium_s as f32) <= max_labels {
            return lvl;
        }
    }
    *GRID_LEVELS.last().unwrap()
}

pub(super) fn paint_timeline_text(
    info: &Info,
    canvas: Rect,
    options: &Options,
    level: GridLevel,
    fixed_timeline_y: f32,
    alpha_multiplier: f32,
    zoom_factor: f32,
) -> Vec<egui::Shape> {
    let mut shapes = vec![];
    let big_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.5..=1.0);
    let medium_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.1..=0.5);

    // Start at the first grid line at or before the visible left edge.
    let visible_left_s = info.start_s
        - ((options.sideways_pan_in_points / info.usable_width()) * options.canvas_width_s) as i64;
    let mut grid_s = (visible_left_s / level.step_s) * level.step_s;

    loop {
        let line_x = info.point_from_s(options, grid_s);

        if line_x > canvas.max.x {
            break;
        }

        if canvas.min.x <= line_x {
            let big_line   = grid_s % level.big_s   == 0;
            let medium_line = grid_s % level.medium_s == 0;

            let text_alpha = if big_line {
                big_alpha
            } else if medium_line {
                medium_alpha
            } else {
                0.0
            };

            if text_alpha > 0.0 {
                let (date_str, time_str) = grid_text(grid_s, level.medium_s < 3600);
                let text_x = line_x + 4.0;
                let text_color = if info.ctx.style().visuals.dark_mode {
                    Color32::from(Rgba::from_white_alpha(
                        (text_alpha * alpha_multiplier * 2.0).min(1.0),
                    ))
                } else {
                    Color32::from(Rgba::from_black_alpha(
                        (text_alpha * alpha_multiplier * 2.0).min(1.0),
                    ))
                };

                info.painter.fonts(|f| {
                    shapes.push(egui::Shape::text(
                        f,
                        pos2(text_x, fixed_timeline_y),
                        Align2::LEFT_TOP,
                        &date_str,
                        info.font_id.clone(),
                        text_color,
                    ));
                    if let Some(t) = time_str {
                        shapes.push(egui::Shape::text(
                            f,
                            pos2(text_x, fixed_timeline_y + info.text_height),
                            Align2::LEFT_TOP,
                            &t,
                            info.font_id.clone(),
                            text_color,
                        ));
                    }
                });
            }
        }

        grid_s += level.step_s;
    }

    shapes
}

pub(super) fn paint_timeline_text_on_top(
    info: &Info,
    options: &Options,
    fixed_timeline_y: f32,
    gutter_width: f32,
) {
    let max_lines = info.usable_width() / 13.0;
    let level = pick_grid_level(options.canvas_width_s, max_lines);

    let theme_colors = get_theme_colors(&info.ctx.style());

    let alpha_multiplier = if info.ctx.style().visuals.dark_mode { 0.3 } else { 0.8 };

    let num_tiny_lines = options.canvas_width_s / (level.step_s as f32);
    let zoom_factor = remap_clamp(num_tiny_lines, (0.1 * max_lines)..=max_lines, 1.0..=0.0);

    let bg_rect = Rect::from_min_size(
        pos2(info.canvas.min.x + gutter_width, fixed_timeline_y),
        egui::vec2(info.usable_width(), info.text_height + 5.0),
    );
    info.painter
        .rect_filled(bg_rect, 0.0, theme_colors.background_timeline);

    let timeline_text = paint_timeline_text(
        info,
        info.canvas,
        options,
        level,
        fixed_timeline_y,
        alpha_multiplier,
        zoom_factor,
    );

    for shape in timeline_text {
        info.painter.add(shape);
    }
}

pub(super) fn paint_timeline(
    info: &Info,
    canvas: Rect,
    options: &Options,
    _start_s: i64,
    _gutter_width: f32,
) -> Vec<egui::Shape> {
    let mut shapes = vec![];
    let theme_colors = get_theme_colors(&info.ctx.style());

    let alpha_multiplier = if info.ctx.style().visuals.dark_mode { 0.3 } else { 0.8 };

    let max_lines = info.usable_width() / 13.0;
    let level = pick_grid_level(options.canvas_width_s, max_lines);

    let num_tiny_lines = options.canvas_width_s / (level.step_s as f32);
    let zoom_factor = remap_clamp(num_tiny_lines, (0.1 * max_lines)..=max_lines, 1.0..=0.0);
    let zoom_factor = zoom_factor * zoom_factor;
    let big_alpha   = remap_clamp(zoom_factor, 0.0..=1.0, 0.5..=1.0);
    let medium_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.1..=0.5);
    let tiny_alpha  = remap_clamp(zoom_factor, 0.0..=1.0, 0.0..=0.1);

    // Start at the first grid line at or before the visible left edge.
    let visible_left_s = info.start_s
        - ((options.sideways_pan_in_points / info.usable_width()) * options.canvas_width_s) as i64;
    let mut grid_s = (visible_left_s / level.step_s) * level.step_s;

    loop {
        let line_x = info.point_from_s(options, grid_s);

        if line_x > canvas.max.x {
            break;
        }

        if canvas.min.x <= line_x {
            let big_line    = grid_s % level.big_s    == 0;
            let medium_line = grid_s % level.medium_s == 0;

            let line_alpha = if big_line {
                big_alpha
            } else if medium_line {
                medium_alpha
            } else {
                tiny_alpha
            };

            shapes.push(egui::Shape::line_segment(
                [pos2(line_x, canvas.min.y), pos2(line_x, canvas.max.y)],
                Stroke::new(
                    1.0,
                    theme_colors
                        .line
                        .linear_multiply(line_alpha * alpha_multiplier),
                ),
            ));
        }

        grid_s += level.step_s;
    }

    shapes
}

pub(super) fn paint_current_time_line(
    info: &Info,
    options: &Options,
    canvas: Rect,
    _gutter_width: f32,
) -> egui::Shape {
    let current_time = chrono::Utc::now().timestamp();
    let line_x = info.point_from_s(options, current_time);

    egui::Shape::line_segment(
        [pos2(line_x, canvas.min.y), pos2(line_x, canvas.max.y)],
        Stroke::new(2.0, options.now_line_color),
    )
}

fn grid_text(ts: i64, split: bool) -> (String, Option<String>) {
    if ts == 0 {
        return ("N/A".to_string(), None);
    }
    let Some(dt) = DateTime::from_timestamp(ts, 0) else {
        return ("Invalid timestamp".to_string(), None);
    };
    let local = dt.with_timezone(&Local);
    if split {
        (
            local.format("%Y-%m-%d").to_string(),
            Some(local.format("%H:%M:%S").to_string()),
        )
    } else {
        (local.format("%Y-%m-%d %H:%M:%S").to_string(), None)
    }
}

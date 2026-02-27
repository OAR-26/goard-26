use super::theme::get_theme_colors;
use super::types::{Info, Options};
use chrono::{DateTime, Local};
use egui::{pos2, remap_clamp, Align2, Color32, Rgba, Rect, Stroke};

pub(super) fn paint_timeline_text(
    info: &Info,
    canvas: Rect,
    options: &Options,
    grid_spacing_minutes: i64,
    fixed_timeline_y: f32,
    alpha_multiplier: f32,
    zoom_factor: f32,
) -> Vec<egui::Shape> {
    let mut shapes = vec![];
    let big_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.5..=1.0);
    let medium_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.1..=0.5);
    let mut grid_s = 0;

    loop {
        let line_x = info.point_from_s(options, grid_s);

        if line_x > canvas.max.x {
            break;
        }

        if canvas.min.x <= line_x {
            let big_line = grid_s % (grid_spacing_minutes * 20) == 0;
            let medium_line = grid_s % (grid_spacing_minutes * 10) == 0;

            let text_alpha = if big_line {
                big_alpha
            } else if medium_line {
                medium_alpha
            } else {
                0.0
            };

            if text_alpha > 0.0 {
                let text = grid_text(grid_s);
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
                        &text,
                        info.font_id.clone(),
                        text_color,
                    ));
                });
            }
        }

        grid_s += grid_spacing_minutes;
    }

    shapes
}

pub(super) fn paint_timeline_text_on_top(
    info: &Info,
    options: &Options,
    fixed_timeline_y: f32,
    gutter_width: f32,
) {
    let max_lines = info.usable_width() / 4.0;
    let mut grid_spacing_minutes = 180;
    while options.canvas_width_s / (grid_spacing_minutes as f32) > max_lines {
        grid_spacing_minutes *= 10;
    }

    let theme_colors = get_theme_colors(&info.ctx.style());

    let alpha_multiplier = if info.ctx.style().visuals.dark_mode { 0.3 } else { 0.8 };

    let num_tiny_lines = options.canvas_width_s / (grid_spacing_minutes as f32);
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
        grid_spacing_minutes,
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

    let max_lines = info.usable_width() / 4.0;
    let mut grid_spacing_minutes = 180;

    while options.canvas_width_s / (grid_spacing_minutes as f32) > max_lines {
        grid_spacing_minutes *= 10;
    }

    let num_tiny_lines = options.canvas_width_s / (grid_spacing_minutes as f32);
    let zoom_factor = remap_clamp(num_tiny_lines, (0.1 * max_lines)..=max_lines, 1.0..=0.0);
    let zoom_factor = zoom_factor * zoom_factor;
    let big_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.5..=1.0);
    let medium_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.1..=0.5);
    let tiny_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.0..=0.1);

    let mut grid_s = 0;

    loop {
        let line_x = info.point_from_s(options, grid_s);

        if line_x > canvas.max.x {
            break;
        }

        if canvas.min.x <= line_x {
            let big_line = grid_s % (grid_spacing_minutes * 20) == 0;
            let medium_line = grid_s % (grid_spacing_minutes * 10) == 0;

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

        grid_s += grid_spacing_minutes;
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
        Stroke::new(2.0, Color32::RED),
    )
}

fn grid_text(ts: i64) -> String {
    if ts == 0 {
        "N/A".to_string()
    } else if let Some(dt) = DateTime::from_timestamp(ts, 0) {
        dt.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    } else {
        "Invalid timestamp".to_string()
    }
}

use super::types::{Info, Options};
use egui::{lerp, PointerButton, Response};

pub(super) fn interact_with_canvas(options: &mut Options, response: &Response, info: &Info) {
    if response.drag_delta().x != 0.0 {
        options.sideways_pan_in_points += response.drag_delta().x;
        options.zoom_to_relative_s_range = None;
    }

    if response.hovered() {
        if info.ctx.input(|i| i.smooth_scroll_delta.x != 0.0) {
            options.sideways_pan_in_points += info.ctx.input(|i| i.smooth_scroll_delta.x);
            options.zoom_to_relative_s_range = None;
        }

        let mut zoom_factor = info.ctx.input(|i| i.zoom_delta_2d().x);

        if response.dragged_by(PointerButton::Secondary) {
            zoom_factor *= (response.drag_delta().y * 0.01).exp();
        }

        if zoom_factor != 1.0 {
            let new_width = options.canvas_width_s / zoom_factor;

            let max_canvas_width = 2 * 24 * 60 * 60; // 2 days in seconds
            if new_width <= max_canvas_width as f32 {
                options.canvas_width_s = new_width;

                if let Some(mouse_pos) = response.hover_pos() {
                    let origin_x = info.canvas.min.x + info.gutter_width;
                    let zoom_center = mouse_pos.x - origin_x;
                    options.sideways_pan_in_points =
                        (options.sideways_pan_in_points - zoom_center) * zoom_factor + zoom_center;
                }
            }
            options.zoom_to_relative_s_range = None;
        }
    }

    if response.double_clicked() {
        options.zoom_to_relative_s_range = Some((
            info.ctx.input(|i| i.time),
            (0., (info.stop_s - info.start_s) as f64),
        ));
    }

    if let Some((start_time, (start_s, end_s))) = options.zoom_to_relative_s_range {
        const ZOOM_DURATION: f32 = 0.75;
        let t = (info.ctx.input(|i| i.time - start_time) as f32 / ZOOM_DURATION).min(1.0);

        let canvas_width = info.usable_width();

        let target_canvas_width_s = (end_s - start_s) as f32;
        let target_pan_in_points = -canvas_width * start_s as f32 / target_canvas_width_s;

        options.canvas_width_s = lerp(
            options.canvas_width_s.recip()..=target_canvas_width_s.recip(),
            t,
        )
        .recip();
        options.sideways_pan_in_points = lerp(options.sideways_pan_in_points..=target_pan_in_points, t);

        if t >= 1.0 {
            options.zoom_to_relative_s_range = None;
        }

        info.ctx.request_repaint();
    }
}

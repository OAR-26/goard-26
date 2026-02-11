use crate::models::data_structure::job::Job;
use crate::models::data_structure::resource::ResourceState;
use crate::views::components::gantt_aggregate_by::AggregateBy;
use crate::views::components::gantt_job_color::JobColor;
use egui::{FontId, Rect, Response};

pub(super) const GUTTER_WIDTH: f32 = 200.0;

pub(super) struct Info {
    pub(super) ctx: egui::Context,
    pub(super) canvas: Rect,
    pub(super) response: Response,
    pub(super) painter: egui::Painter,
    pub(super) text_height: f32,
    pub(super) start_s: i64,
    pub(super) stop_s: i64,
    pub(super) font_id: FontId,
    pub(super) gutter_width: f32,
}

impl Info {
    pub(super) fn usable_width(&self) -> f32 {
        (self.canvas.width() - self.gutter_width).max(1.0)
    }

    pub(super) fn point_from_s(&self, options: &Options, ns: i64) -> f32 {
        self.canvas.min.x
            + self.gutter_width
            + options.sideways_pan_in_points
            + self.usable_width() * ((ns - self.start_s) as f32) / options.canvas_width_s
    }
}

pub struct Options {
    pub canvas_width_s: f32,
    pub sideways_pan_in_points: f32,
    pub cull_width: f32,
    pub min_width: f32,
    pub rect_height: f32,
    pub spacing: f32,
    pub rounding: f32,
    pub aggregate_by: AggregateBy,
    pub job_color: JobColor,
    pub see_all_res: bool,
    pub current_hovered_job: Option<Job>,
    pub previous_hovered_job: Option<Job>,
    pub current_hovered_resource_state: Option<ResourceState>,
    pub squash_resources: bool,
    pub compact_rows: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub zoom_to_relative_s_range: Option<(f64, (f64, f64))>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            canvas_width_s: 0.0,
            sideways_pan_in_points: 0.0,
            cull_width: 0.0,
            min_width: 1.0,
            rect_height: 16.0,
            spacing: 0.0,
            rounding: 4.0,
            aggregate_by: Default::default(),
            job_color: Default::default(),
            zoom_to_relative_s_range: None,
            current_hovered_job: None,
            previous_hovered_job: None,
            squash_resources: false,
            see_all_res: false,
            current_hovered_resource_state: None,
            compact_rows: true,
        }
    }
}

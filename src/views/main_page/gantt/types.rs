use crate::models::data_structure::job::Job;
use crate::models::data_structure::resource::{DeadInterval, ResourceState};
use crate::views::components::gantt_job_color::JobColor;
use egui::{FontId, Rect, Response};

pub(super) const GUTTER_WIDTH: f32 = 200.0;

/// Width of each stripe column in the gutter (one per hierarchy level).
pub(super) const STRIPE_W: f32 = 12.0;

pub(super) fn gutter_stripes_total_w(n_levels: usize) -> f32 {
    n_levels as f32 * STRIPE_W
}

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

/// Optional equality filter applied to every resource before grouping.
/// `exclude = false` → keep only resources where field == value.
/// `exclude = true`  → keep only resources where field != value.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub(super) struct ResourceFilter {
    pub field: String,
    pub value: String,
    #[serde(default)]
    pub exclude: bool,
}

/// A reusable preset that defines how a leaf resource is described in tooltips.
/// `name` is shown as the label prefix (e.g. "host: node1.cluster");
/// `fields` lists strata fields to display below it.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct LeafInfoPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub fields: Vec<String>,
}

pub struct Options {
    pub canvas_width_s: f32,
    pub sideways_pan_in_points: f32,
    pub cull_width: f32,
    pub min_width: f32,
    pub rect_height: f32,
    pub spacing: f32,
    pub rounding: f32,
    /// Ordered list of resource field names that define the hierarchy levels,
    /// outermost first (e.g. ["cluster", "host"]).
    pub levels: Vec<String>,
    pub job_color: JobColor,
    pub current_hovered_job: Option<Job>,
    pub previous_hovered_job: Option<Job>,
    pub current_hovered_resource_state: Option<ResourceState>,
    pub current_hovered_resource_label: Option<String>,
    pub current_hovered_dead_interval: Option<DeadInterval>,
    /// Some(true) = full drain, Some(false) = partial drain row hovered.
    pub current_hovered_drain: Option<bool>,
    /// Label of the leaf-level item currently under the mouse (used to highlight
    /// matching stripes when hovering a job bar).
    pub hovered_leaf_label: Option<String>,
    pub compact_rows: bool,
    pub resource_filter: Option<ResourceFilter>,
    /// Optional template for the leaf-row label.  `{field}` placeholders are
    /// replaced by the corresponding value from the ancestor path context or
    /// the leaf key itself.  Example: `"{type}/{vlan}"`.
    pub leaf_label_template: Option<String>,
    /// When true, groups at every level are sorted by their minimum pre-computed
    /// leaf label rather than by the raw grouping key.  Use when grouping keys
    /// are opaque IDs (e.g. slash_* subnet blocks) that don't sort meaningfully.
    pub sort_by_label: bool,
    /// Resolved preset for the current view — drives tooltip label and field list.
    pub leaf_info_preset: Option<LeafInfoPreset>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub zoom_to_relative_s_range: Option<(f64, (f64, f64))>,

    // ── From GanttConfig (set once on startup) ────────────────────────────────
    pub row_height_min: f32,
    pub row_height_max: f32,
    pub scroll_zoom_sensitivity: f32,
    pub drag_zoom_sensitivity: f32,
    pub zoom_max_seconds: f32,
    pub zoom_min_seconds: f32,
    pub zoom_animation_duration: f64,
    pub hatch_spacing: f32,
    pub job_label_min_width: f32,
    /// Job field rendered inside bar labels (e.g. "id", "owner", "name"). Default: "id".
    pub job_label_field: String,
    pub now_line_color: egui::Color32,
    /// Step durations in seconds, smallest to largest. Each drives one ◀/▶ button pair.
    pub nav_steps: Vec<i64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            canvas_width_s: 0.0,
            sideways_pan_in_points: 0.0,
            cull_width: 0.0,
            min_width: 1.0,
            rect_height: 20.0,
            spacing: 0.0,
            rounding: 4.0,
            levels: vec!["site".to_string(), "cluster".to_string(), "host".to_string()],
            job_color: Default::default(),
            zoom_to_relative_s_range: None,
            current_hovered_job: None,
            previous_hovered_job: None,
            current_hovered_resource_state: None,
            current_hovered_resource_label: None,
            current_hovered_dead_interval: None,
            current_hovered_drain: None,
            hovered_leaf_label: None,
            compact_rows: true,
            resource_filter: None,
            leaf_label_template: None,
            sort_by_label: false,
            leaf_info_preset: None,
            row_height_min: 8.0,
            row_height_max: 80.0,
            scroll_zoom_sensitivity: 0.0025,
            drag_zoom_sensitivity: 0.01,
            zoom_max_seconds: (2 * 24 * 60 * 60) as f32,
            zoom_min_seconds: 5.0,
            zoom_animation_duration: 0.75,
            hatch_spacing: 10.0,
            job_label_min_width: 30.0,
            job_label_field: "id".to_string(),
            now_line_color: egui::Color32::RED,
            nav_steps: vec![86_400, 7 * 86_400],
        }
    }
}

mod canvas;
mod interaction;
pub(crate) mod jobs;
mod labels;
mod theme;
mod timeline;
mod types;
mod xy_plot;
mod panels;

use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::utils::{cluster_names, host_names, all_resource_ids};
use crate::views::view::View;
use crate::{
    models::data_structure::{
        application_context::ApplicationContext,
        job::{Job, JobState},
    },
    views::components::job_details::JobDetailsWindow,
};
use chrono::{Local, TimeZone};
use eframe::egui;
use egui::{Color32, FontId, Frame, RichText, ScrollArea, Sense, Shape, TextStyle};

use panels::{
    CreatePresetPanel, CreateViewPanel, EditPresetPanel,
    EditViewPanel, XyPanelState, PresetPanelAction, ViewFormAction,
};

// ---------------------------------------------------------------------------
// View hierarchy configuration (loaded from views.json)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct GanttView {
    name: String,
    levels: Vec<String>,
    #[serde(default)]
    filter: Option<types::ResourceFilter>,
    #[serde(default)]
    leaf_label_template: Option<String>,
    #[serde(default)]
    sort_by_label: bool,
    /// Fields shown in the status bar (e.g. ["cluster","host"]). Defaults to [last level] if empty.
    #[serde(default)]
    summary_fields: Vec<String>,
    /// ID of the `LeafInfoPreset` that defines this view's tooltip label and fields.
    #[serde(default)]
    leaf_infos: Option<String>,
    // Backward-compat: read from old JSON, never written back.
    #[serde(default, skip_serializing)]
    leaf_display_name: String,
    #[serde(default, skip_serializing)]
    leaf_hover_details: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
struct ViewsConfig {
    #[serde(default)]
    views: Vec<GanttView>,
    #[serde(default)]
    leaf_info_presets: Vec<types::LeafInfoPreset>,
}

fn load_views_config() -> ViewsConfig {
    let fallback = ViewsConfig {
        views: vec![
            GanttView {
                name: "Compute: site → cluster → host".to_string(),
                levels: vec!["site".to_string(), "cluster".to_string(), "host".to_string()],
                filter: None,
                leaf_label_template: Some("{host|short}".to_string()),
                sort_by_label: false,
                summary_fields: vec!["cluster".to_string(), "host".to_string()],
                leaf_infos: None,
                leaf_display_name: "Host".to_string(),
                leaf_hover_details: true,
            },
            GanttView {
                name: "Network: site → type → vlan".to_string(),
                levels: vec!["site".to_string(), "type".to_string(), "vlan".to_string()],
                filter: None,
                leaf_label_template: Some("{type}/{vlan}".to_string()),
                sort_by_label: false,
                summary_fields: vec!["vlan".to_string()],
                leaf_infos: None,
                leaf_display_name: "VLAN".to_string(),
                leaf_hover_details: false,
            },
        ],
        leaf_info_presets: vec![],
    };
    #[cfg(target_arch = "wasm32")]
    let content = include_str!("../../../../views.json").to_string();
    #[cfg(not(target_arch = "wasm32"))]
    let Ok(content) = std::fs::read_to_string("ganttza/views.json") else { return fallback; };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) else { return fallback; };
    if val.is_array() {
        // Old format: bare array of views — no presets
        let views: Vec<GanttView> = serde_json::from_value(val).unwrap_or(fallback.views.clone());
        ViewsConfig { views, leaf_info_presets: vec![] }
    } else {
        serde_json::from_value(val).unwrap_or(fallback)
    }
}

fn resolve_leaf_preset<'a>(
    presets: &'a [types::LeafInfoPreset],
    id: &Option<String>,
) -> Option<&'a types::LeafInfoPreset> {
    presets.iter().find(|p| Some(&p.id) == id.as_ref())
}

/// Build a transient preset from old `leaf_display_name` / `leaf_hover_details` fields.
fn backward_compat_preset(view: &GanttView) -> Option<types::LeafInfoPreset> {
    if view.leaf_display_name.is_empty() { return None; }
    Some(types::LeafInfoPreset {
        id: String::new(),
        name: view.leaf_display_name.clone(),
        fields: if view.leaf_hover_details {
            vec![
                "cluster".to_string(), "network_address".to_string(),
                "comment".to_string(), "nodemodel".to_string(),
                "cputype".to_string(), "resource_id".to_string(),
            ]
        } else {
            vec![]
        },
    })
}

fn save_views_config(views: &[GanttView], presets: &[types::LeafInfoPreset]) {
    #[derive(serde::Serialize)]
    struct Out<'a> {
        views: &'a [GanttView],
        leaf_info_presets: &'a [types::LeafInfoPreset],
    }
    if let Ok(json) = serde_json::to_string_pretty(&Out { views, leaf_info_presets: presets }) {
        let _ = std::fs::write("ganttza/views.json", json);
    }
}

use self::types::{gutter_stripes_total_w, Info, Options, GUTTER_WIDTH};


fn cfg_field_colors(
    src: &std::collections::HashMap<String, std::collections::HashMap<String, crate::models::data_structure::gantt_config::RgbColor>>,
) -> std::collections::HashMap<String, std::collections::HashMap<String, (egui::Color32, egui::Color32)>> {
    let darker = |c: u8| -> u8 { ((c as f32 * 0.8) as u8).max(1) };
    src.iter().map(|(field, values)| {
        let mapped = values.iter().map(|(val, &c)| {
            let fill  = egui::Color32::from_rgb(c.0, c.1, c.2);
            let hover = egui::Color32::from_rgb(darker(c.0), darker(c.1), darker(c.2));
            (val.clone(), (fill, hover))
        }).collect();
        (field.clone(), mapped)
    }).collect()
}

fn compute_gutter_width(
    ctx: &egui::Context,
    base_font: &FontId,
    options: &Options,
    app: &ApplicationContext,
) -> f32 {
    let n_total = options.levels.len();
    let stripes_w = gutter_stripes_total_w(n_total);

    let max_label = jobs::max_leaf_label(app, &options.levels, options.leaf_label_template.as_deref());
    let font_leaf = FontId::proportional((base_font.size).max(11.0));
    let label_text_w = ctx
        .fonts(|f| f.layout_no_wrap(max_label, font_leaf, Color32::BLACK).size().x);
    // 4px left pad + 4px right pad; minimum 40px label area so an empty view isn't invisible.
    let label_w = (label_text_w + 8.0).max(40.0);

    (label_w + stripes_w).min(app.prefs.gantt_config.gutter_max_width)
}

pub struct GanttChart {
    options: Options,
    job_details_windows: Vec<JobDetailsWindow>,
    initial_start_s: Option<i64>,
    initial_end_s: Option<i64>,
    /// Fixed hierarchy levels pushed by the binary (e.g. a file type with a
    /// mandatory layout) — when set, takes precedence over the View dropdown.
    /// Core has no idea why; it just renders these levels instead.
    hierarchy_override: Option<Vec<String>>,
    /// Visible (start_s, end_s) for XY-only sources (no Gantt canvas to
    /// derive it from pixel-pan). Set directly by nav controls / drag; a
    /// binary that wants to persist/restore this across switches owns that.
    current_xy_visible: Option<(i64, i64)>,

    gantt_views: Vec<GanttView>,
    current_view_index: usize,
    leaf_info_presets: Vec<types::LeafInfoPreset>,

    last_canvas_usable_width_px: f32,
    pending_navigation_refresh: bool,
    delete_view_confirm: Option<usize>,
    delete_preset_confirm: Option<String>,

    xy_panel: XyPanelState,
    create_view: CreateViewPanel,
    create_preset: CreatePresetPanel,
    edit_view: EditViewPanel,
    edit_preset: EditPresetPanel,

    jump_dialog_open: bool,
    jump_date_str: String,
    jump_time_str: String,
    jump_error: Option<String>,
}

impl Default for GanttChart {
    fn default() -> Self {
        let config = load_views_config();
        let gantt_cfg = crate::models::data_structure::gantt_config::GanttConfig::load();
        let mut options = Options::default();
        options.canvas_width_s          = gantt_cfg.default_timespan_s as f32;
        options.rect_height             = gantt_cfg.gantt_row_height;
        options.row_height_min          = gantt_cfg.gantt_row_height_min;
        options.row_height_max          = gantt_cfg.gantt_row_height_max;
        options.scroll_zoom_sensitivity = gantt_cfg.scroll_zoom_sensitivity;
        options.drag_zoom_sensitivity   = gantt_cfg.drag_zoom_sensitivity;
        options.zoom_max_seconds        = gantt_cfg.zoom_max_seconds;
        options.zoom_min_seconds        = gantt_cfg.zoom_min_seconds;
        options.zoom_animation_duration = gantt_cfg.zoom_animation_duration;
        options.hatch_spacing           = gantt_cfg.hatch_spacing;
        options.job_block_border        = gantt_cfg.job_block_border;
        options.job_block_border_width  = gantt_cfg.job_block_border_width;
        options.job_label_min_width     = gantt_cfg.job_label_min_width;
        options.job_label_field         = gantt_cfg.job_label_field.clone();
        options.job_color_field         = gantt_cfg.job_color_field.clone();
        options.field_colors         = cfg_field_colors(&gantt_cfg.field_colors);
        options.nav_steps = gantt_cfg.nav_steps_s();
        let mut xy_panel = XyPanelState::default();
        xy_panel.panel_height = gantt_cfg.xy_panel_height;
        let now_color = egui::Color32::from_rgb(
            gantt_cfg.now_line_color.0, gantt_cfg.now_line_color.1, gantt_cfg.now_line_color.2);
        xy_panel.now_line_color = now_color;
        options.now_line_color = now_color;
        if let Some(first) = config.views.first() {
            options.levels = first.levels.clone();
            options.resource_filter = first.filter.clone();
            options.leaf_label_template = first.leaf_label_template.clone();
            options.sort_by_label = first.sort_by_label;
            options.leaf_info_preset = resolve_leaf_preset(&config.leaf_info_presets, &first.leaf_infos)
                .cloned()
                .or_else(|| backward_compat_preset(first));
        }
        GanttChart {
            options,
            job_details_windows: Vec::new(),
            initial_start_s: None,
            initial_end_s: None,
            hierarchy_override: None,
            current_xy_visible: None,
            last_canvas_usable_width_px: 1.0,
            pending_navigation_refresh: false,
            delete_view_confirm: None,
            delete_preset_confirm: None,
            gantt_views: config.views,
            current_view_index: 0,
            leaf_info_presets: config.leaf_info_presets,
            xy_panel,
            create_view: CreateViewPanel::default(),
            create_preset: CreatePresetPanel::default(),
            edit_view: EditViewPanel::default(),
            edit_preset: EditPresetPanel::default(),
            jump_dialog_open: false,
            jump_date_str: String::new(),
            jump_time_str: String::new(),
            jump_error: None,
        }
    }
}

impl GanttChart {
    pub fn render_compact_toolbar(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        // First-ever frame bootstrap — a binary that manages multiple sources
        // is expected to call `apply_view_snapshot` right after switching, which
        // overrides this; this only matters before that's ever happened (e.g.
        // a binary with a single perpetual source, like a live feed).
        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        // Every frame: apply the binary's fixed hierarchy override if present,
        // else follow whatever Gantt View is currently selected.
        if let Some(levels) = &self.hierarchy_override {
            self.options.levels = levels.clone();
            self.options.resource_filter = None;
            self.options.leaf_label_template = None;
        } else if let Some(view) = self.gantt_views.get(self.current_view_index) {
            self.options.levels = view.levels.clone();
            self.options.resource_filter = view.filter.clone();
            self.options.leaf_label_template = view.leaf_label_template.clone();
            self.options.sort_by_label = view.sort_by_label;
            self.options.leaf_info_preset = resolve_leaf_preset(&self.leaf_info_presets, &view.leaf_infos)
                .cloned()
                .or_else(|| backward_compat_preset(view));
        }

        let supports_hierarchy = app.show_hierarchy_controls;

        // "View" dropdown in the top toolbar — mirrors the tab row below
        let current_view_name = self.gantt_views
            .get(self.current_view_index)
            .map(|v| v.name.as_str())
            .unwrap_or("View");
        if supports_hierarchy {
            ui.menu_button(format!("View: {}", current_view_name), |ui| {
                ui.set_min_width(220.0);
                for i in 0..self.gantt_views.len() {
                    let is_active = self.current_view_index == i;
                    let name = self.gantt_views[i].name.clone();
                    ui.horizontal(|ui| {
                        if ui.selectable_label(is_active, &name).clicked() {
                            self.current_view_index = i;
                            self.options.levels = self.gantt_views[i].levels.clone();
                            self.options.resource_filter = self.gantt_views[i].filter.clone();
                            self.options.leaf_label_template = self.gantt_views[i].leaf_label_template.clone();
                            self.options.sort_by_label = self.gantt_views[i].sort_by_label;
                            self.options.leaf_info_preset =
                                resolve_leaf_preset(&self.leaf_info_presets, &self.gantt_views[i].leaf_infos)
                                    .cloned()
                                    .or_else(|| backward_compat_preset(&self.gantt_views[i]));
                            ui.close_menu();
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗑").on_hover_text("Delete view").clicked() {
                                self.delete_view_confirm = Some(i);
                                ui.close_menu();
                            }
                            if ui.small_button("✏").on_hover_text("Edit view").clicked() {
                                let v = self.gantt_views[i].clone();
                                self.edit_view.open_for(i, &v);
                                ui.close_menu();
                            }
                        });
                    });
                }
                ui.separator();
                if ui.button("+ Create view").clicked() {
                    self.create_view.reset_and_open();
                    ui.close_menu();
                }
            });
        }

        ui.add_space(6.0);

        // Quick timeline navigation
        fn fmt_nav(s: i64) -> String {
            if s % (7 * 86_400) == 0  { format!("{}w", s / (7 * 86_400)) }
            else if s % 86_400 == 0   { format!("{}d", s / 86_400) }
            else if s % 3_600 == 0    { format!("{}h", s / 3_600) }
            else if s % 60 == 0       { format!("{}min", s / 60) }
            else                      { format!("{}s", s) }
        }

        let steps = self.options.nav_steps.clone();
        ui.label(RichText::new("Nav:").text_style(TextStyle::Small));
        // ◀ buttons: largest step first (reverse order)
        for &step_s in steps.iter().rev() {
            if ui.small_button(format!("◀ {}", fmt_nav(step_s))).clicked() {
                let (vs, ve) = self.current_visible_window(app);
                self.set_visible_window(app, vs - step_s, ve - step_s);
            }
        }

        // Jump-to button
        if ui.small_button("🕐").on_hover_text("Jump to date/time").clicked() {
            let (vs, ve) = self.current_visible_window(app);
            let center_s = vs + (ve - vs) / 2;
            let dt = Local.timestamp_opt(center_s, 0).single()
                .unwrap_or_else(Local::now);
            self.jump_date_str = dt.format("%Y-%m-%d").to_string();
            self.jump_time_str = dt.format("%H:%M").to_string();
            self.jump_error = None;
            self.jump_dialog_open = true;
        }

        // ▶ buttons: smallest step first (forward order)
        for &step_s in steps.iter() {
            if ui.small_button(format!("{} ▶", fmt_nav(step_s))).clicked() {
                let (vs, ve) = self.current_visible_window(app);
                self.set_visible_window(app, vs + step_s, ve + step_s);
            }
        }

        if ui.small_button(t!("app.gantt.now")).clicked() {
            if app.show_gantt() {
                // Smooth animation — Gantt-canvas-only concept.
                self.options.zoom_to_relative_s_range = Some((
                    ui.ctx().input(|i| i.time),
                    (
                        0.,
                        (self.initial_end_s.unwrap() - self.initial_start_s.unwrap()) as f64,
                    ),
                ));
                self.pending_navigation_refresh = true;
            } else {
                let (vs, ve) = self.current_visible_window(app);
                let width = (ve - vs).max(1);
                let now_s = chrono::Utc::now().timestamp();
                self.set_visible_window(app, now_s - width / 2, now_s + width / 2);
            }
        }
    }

    /// True (and clears the flag) if timeline navigation since the last call
    /// (pan/zoom/jump) means fresher data for the visible window would be
    /// useful. Core has no refresh engine of its own — a binary backed by a
    /// live data source can poll this after `render()` and act on it however
    /// it likes; one with no such engine can simply ignore it.
    pub fn take_navigation_refresh_request(&mut self) -> bool {
        std::mem::replace(&mut self.pending_navigation_refresh, false)
    }

    /// Sets the color palette used to cycle through multi-series XY plot lines.
    /// Core falls back to a deterministic per-index color when empty (default).
    pub fn set_xy_series_colors(&mut self, colors: Vec<[u8; 3]>) {
        self.xy_panel.series_colors = colors;
    }

    /// Current visible (start_s, end_s) window, regardless of whether this
    /// tab is showing the Gantt canvas (pixel-pan based) or an energy-only
    /// view (tracked directly in seconds) — nav controls shouldn't care which.
    fn current_visible_window(&self, app: &ApplicationContext) -> (i64, i64) {
        if app.show_gantt() {
            let usable = self.last_canvas_usable_width_px.max(1.0);
            let init_s = self.initial_start_s.unwrap_or_else(|| chrono::Utc::now().timestamp());
            let pan_ratio = self.options.sideways_pan_in_points / usable;
            let vs = init_s - (pan_ratio * self.options.canvas_width_s) as i64;
            let ve = vs + self.options.canvas_width_s as i64;
            (vs, ve)
        } else {
            let default_vs = self.initial_start_s.unwrap_or_else(|| app.get_start_date().timestamp());
            let default_ve = self.initial_end_s.unwrap_or_else(|| app.get_end_date().timestamp());
            self.current_xy_visible.unwrap_or((default_vs, default_ve))
        }
    }

    /// Sets the visible window, writing to whichever representation this
    /// tab actually reads (Gantt pixel-pan, or the energy-only seconds field),
    /// and flags that a refresh would be useful for the new window.
    fn set_visible_window(&mut self, app: &ApplicationContext, new_vs: i64, new_ve: i64) {
        if app.show_gantt() {
            let usable = self.last_canvas_usable_width_px.max(1.0);
            let init_s = self.initial_start_s.unwrap_or(new_vs);
            let canvas_w = (new_ve - new_vs).max(1) as f32;
            self.options.canvas_width_s = canvas_w;
            self.options.sideways_pan_in_points = (init_s as f32 - new_vs as f32) * usable / canvas_w;
            self.options.zoom_to_relative_s_range = None;
        } else {
            self.current_xy_visible = Some((new_vs, new_ve));
        }
        self.pending_navigation_refresh = true;
    }

    /// Sets a fixed hierarchy override (e.g. a file type with a mandatory
    /// layout). `None` reverts to following the selected Gantt View.
    pub fn set_hierarchy_override(&mut self, levels: Option<Vec<String>>) {
        self.hierarchy_override = levels;
    }

    /// Captures everything about the current view worth remembering across
    /// a source switch — zoom/pan, selected Gantt View, energy panel state,
    /// and the hierarchy override. Core has no notion of "tabs"; a binary
    /// that wants per-tab memory captures one of these before switching away
    /// and applies it (or a freshly-built default) after switching to a
    /// source it hasn't seen yet.
    pub fn capture_view_snapshot(&self) -> GanttViewSnapshot {
        GanttViewSnapshot {
            initial_start_s: self.initial_start_s.unwrap_or(0),
            initial_end_s: self.initial_end_s.unwrap_or(0),
            canvas_width_s: self.options.canvas_width_s,
            sideways_pan_in_points: self.options.sideways_pan_in_points,
            rect_height: self.options.rect_height,
            current_view_index: self.current_view_index,
            xy_y_bounds: self.xy_panel.y_bounds,
            xy_fit_to_figure: self.xy_panel.fit_to_figure,
            xy_panel_height: self.xy_panel.panel_height,
            xy_visible_range: self.current_xy_visible,
            hierarchy_override: self.hierarchy_override.clone(),
        }
    }

    /// Applies a previously captured (or freshly built) snapshot.
    pub fn apply_view_snapshot(&mut self, snap: &GanttViewSnapshot) {
        self.initial_start_s = Some(snap.initial_start_s);
        self.initial_end_s = Some(snap.initial_end_s);
        self.options.canvas_width_s = snap.canvas_width_s;
        self.options.sideways_pan_in_points = snap.sideways_pan_in_points;
        self.options.rect_height = snap.rect_height;
        self.options.zoom_to_relative_s_range = None;
        self.current_view_index = snap.current_view_index.min(self.gantt_views.len().saturating_sub(1));
        self.xy_panel.y_bounds = snap.xy_y_bounds;
        self.xy_panel.fit_to_figure = snap.xy_fit_to_figure;
        self.xy_panel.panel_height = snap.xy_panel_height;
        self.current_xy_visible = snap.xy_visible_range;
        self.hierarchy_override = snap.hierarchy_override.clone();
    }
}

/// See `GanttChart::capture_view_snapshot`/`apply_view_snapshot`.
#[derive(Clone, Debug)]
pub struct GanttViewSnapshot {
    pub initial_start_s: i64,
    pub initial_end_s: i64,
    pub canvas_width_s: f32,
    pub sideways_pan_in_points: f32,
    pub rect_height: f32,
    pub current_view_index: usize,
    pub xy_y_bounds: Option<(f64, f64)>,
    pub xy_fit_to_figure: bool,
    pub xy_panel_height: f32,
    pub xy_visible_range: Option<(i64, i64)>,
    pub hierarchy_override: Option<Vec<String>>,
}

impl View for GanttChart {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        // Apply config changes triggered by ConfigPanel's Apply button.
        if app.prefs.config_reload_requested {
            app.prefs.config_reload_requested = false;
            let cfg = &app.prefs.gantt_config;
            self.options.row_height_min          = cfg.gantt_row_height_min;
            self.options.row_height_max          = cfg.gantt_row_height_max;
            self.options.scroll_zoom_sensitivity = cfg.scroll_zoom_sensitivity;
            self.options.drag_zoom_sensitivity   = cfg.drag_zoom_sensitivity;
            self.options.zoom_max_seconds        = cfg.zoom_max_seconds;
            self.options.zoom_min_seconds        = cfg.zoom_min_seconds;
            self.options.zoom_animation_duration = cfg.zoom_animation_duration;
            self.options.hatch_spacing           = cfg.hatch_spacing;
            self.options.job_block_border        = cfg.job_block_border;
            self.options.job_block_border_width  = cfg.job_block_border_width;
            self.options.job_label_min_width     = cfg.job_label_min_width;
            self.options.job_label_field         = cfg.job_label_field.clone();
            self.options.job_color_field         = cfg.job_color_field.clone();
            self.options.field_colors         = cfg_field_colors(&cfg.field_colors);
            self.options.rect_height             = cfg.gantt_row_height
                .clamp(cfg.gantt_row_height_min, cfg.gantt_row_height_max);
            self.options.now_line_color = egui::Color32::from_rgb(
                cfg.now_line_color.0, cfg.now_line_color.1, cfg.now_line_color.2);
            self.xy_panel.now_line_color = self.options.now_line_color;
            self.options.nav_steps = cfg.nav_steps_s();
        }
        self.options.job_color.color  = app.prefs.job_color.color.clone();
        self.options.job_color_field  = app.prefs.gantt_config.job_color_field.clone();
        self.options.field_colors = cfg_field_colors(&app.prefs.gantt_config.field_colors);

        // First-ever frame bootstrap — see `render_compact_toolbar`.
        if self.initial_start_s.is_none() {
            self.initial_start_s = Some(app.get_start_date().timestamp());
            self.initial_end_s = Some(app.get_end_date().timestamp());
        }

        // Every frame: enforce the binary's fixed hierarchy override if
        // present, else follow whatever Gantt View is currently selected —
        // regardless of what other code paths (tab clicks, view dropdown)
        // may have set previously.
        if let Some(levels) = &self.hierarchy_override {
            self.options.levels = levels.clone();
            self.options.resource_filter = None;
            self.options.leaf_label_template = None;
        } else if let Some(view) = self.gantt_views.get(self.current_view_index) {
            self.options.levels = view.levels.clone();
            self.options.resource_filter = view.filter.clone();
            self.options.leaf_label_template = view.leaf_label_template.clone();
        }

        // Keep toolbar in sync with current view.
        let view_name = self.gantt_views.get(self.current_view_index)
            .map(|v| v.name.clone())
            .unwrap_or_default();
        if app.prefs.current_gantt_view_name != view_name {
            app.prefs.current_gantt_view_name = view_name;
            app.prefs.current_gantt_view_levels = self.options.levels.clone();
            app.prefs.current_gantt_view_summary_fields = {
                let stored = self.gantt_views
                    .get(self.current_view_index)
                    .map(|v| v.summary_fields.clone())
                    .unwrap_or_default();
                if stored.is_empty() {
                    self.options.levels.last().cloned().into_iter().collect()
                } else {
                    stored
                }
            };
        }

        if app.show_all_resources_row {
            app.data.all_jobs.retain(|j| j.id != 0);
        }

        let selected_cluster_names: Option<Vec<String>> = app.filters.selected_cluster_names.clone();

        let all_hosts = if let Some(filter_clusters) = &selected_cluster_names {
            host_names(&app.data.strata_by_resource_id)
                .into_iter()
                .filter(|h| {
                    app.data.strata_by_resource_id.values()
                        .any(|s| s.host.as_deref() == Some(h.as_str())
                            && s.cluster.as_ref().map(|c| filter_clusters.contains(c)).unwrap_or(false))
                })
                .collect()
        } else {
            host_names(&app.data.strata_by_resource_id)
        };

        let all_clusters = if let Some(filter_clusters) = &selected_cluster_names {
            filter_clusters.clone()
        } else {
            cluster_names(&app.data.strata_by_resource_id)
        };

        let all_resources = if let Some(filter_clusters) = &selected_cluster_names {
            all_resource_ids(&app.data.strata_by_resource_id)
                .into_iter()
                .filter(|rid| {
                    app.data.strata_by_resource_id.get(rid)
                        .and_then(|s| s.cluster.as_ref())
                        .map(|c| filter_clusters.contains(c))
                        .unwrap_or(false)
                })
                .collect()
        } else {
            all_resource_ids(&app.data.strata_by_resource_id)
        };

        if app.show_all_resources_row {
            app.data.all_jobs.push(Job {
                id: 0,
                owner: "all_resources".to_string(),
                state: JobState::Unknown,
                scheduled_start: 0,
                walltime: 0,
                hosts: all_hosts,
                clusters: all_clusters,
                command: String::new(),
                message: None,
                queue: String::new(),
                assigned_resources: all_resources,
                submission_time: 0,
                start_time: 0,
                stop_time: 0,
                exit_code: None,
                main_resource_state: ResourceState::Unknown,
                job_type: String::new(),
                job_types: Vec::new(),
                name: None,
                project: String::new(),
            });
        }

        // ── Dialog panels ─────────────────────────────────────────────────────
        let presets_for_view = self.leaf_info_presets.clone();
        for action in self.create_view.show(ui, &presets_for_view) {
            match action {
                ViewFormAction::Created(view) => {
                    self.gantt_views.push(view);
                    save_views_config(&self.gantt_views, &self.leaf_info_presets);
                }
                ViewFormAction::WantCreatePreset => self.create_preset.reset_and_open(),
                ViewFormAction::WantEditPreset(i) => {
                    if let Some(p) = self.leaf_info_presets.get(i).cloned() {
                        self.edit_preset.open_for(i, &p);
                    }
                }
                ViewFormAction::WantDeletePreset(id) => self.delete_preset_confirm = Some(id),
                ViewFormAction::Applied { .. } => {}
            }
        }

        for action in self.edit_view.show(ui, &presets_for_view) {
            match action {
                ViewFormAction::Applied { idx, view } => {
                    if self.current_view_index == idx {
                        self.options.levels = view.levels.clone();
                        self.options.resource_filter = view.filter.clone();
                        self.options.leaf_label_template = view.leaf_label_template.clone();
                        self.options.sort_by_label = view.sort_by_label;
                        self.options.leaf_info_preset =
                            resolve_leaf_preset(&self.leaf_info_presets, &view.leaf_infos)
                                .cloned()
                                .or_else(|| backward_compat_preset(&view));
                    }
                    self.gantt_views[idx] = view;
                    save_views_config(&self.gantt_views, &self.leaf_info_presets);
                }
                ViewFormAction::WantCreatePreset => self.create_preset.reset_and_open(),
                ViewFormAction::WantEditPreset(i) => {
                    if let Some(p) = self.leaf_info_presets.get(i).cloned() {
                        self.edit_preset.open_for(i, &p);
                    }
                }
                ViewFormAction::WantDeletePreset(id) => self.delete_preset_confirm = Some(id),
                ViewFormAction::Created(_) => {}
            }
        }

        if let Some(action) = self.create_preset.show(ui) {
            if let PresetPanelAction::Saved(preset) = action {
                let id = preset.id.clone();
                self.leaf_info_presets.push(preset);
                if self.create_view.open { self.create_view.set_preset_id(Some(id.clone())); }
                if self.edit_view.open { self.edit_view.set_preset_id(Some(id)); }
                save_views_config(&self.gantt_views, &self.leaf_info_presets);
            }
        }

        if let Some(action) = self.edit_preset.show(ui) {
            if let PresetPanelAction::Applied { idx, name, fields } = action {
                self.leaf_info_presets[idx].name = name;
                self.leaf_info_presets[idx].fields = fields;
                let active_preset_id = self.gantt_views
                    .get(self.current_view_index)
                    .and_then(|v| v.leaf_infos.as_deref())
                    .map(str::to_string);
                if active_preset_id.as_deref() == Some(self.leaf_info_presets[idx].id.as_str()) {
                    self.options.leaf_info_preset = Some(self.leaf_info_presets[idx].clone());
                }
                save_views_config(&self.gantt_views, &self.leaf_info_presets);
            }
        }

        if let Some(idx) = self.delete_view_confirm {
            let name = self.gantt_views.get(idx).map(|v| v.name.clone()).unwrap_or_default();
            egui::Window::new("Confirm delete view")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Delete view '{}'?", name));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            if self.current_view_index >= idx && self.current_view_index > 0 {
                                self.current_view_index -= 1;
                            }
                            self.gantt_views.remove(idx);
                            save_views_config(&self.gantt_views, &self.leaf_info_presets);
                            self.delete_view_confirm = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.delete_view_confirm = None;
                        }
                    });
                });
        }

        if self.delete_preset_confirm.is_some() {
            let id = self.delete_preset_confirm.clone().unwrap();
            let name = self.leaf_info_presets.iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| id.clone());
            egui::Window::new("Confirm delete preset")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Delete preset '{}'?", name));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.leaf_info_presets.retain(|p| p.id != id);
                            save_views_config(&self.gantt_views, &self.leaf_info_presets);
                            self.delete_preset_confirm = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.delete_preset_confirm = None;
                        }
                    });
                });
        }

        // ── Jump-to dialog ────────────────────────────────────────────────────
        if self.jump_dialog_open {
            let mut do_jump = false;
            let mut do_cancel = false;
            egui::Window::new("Jump to...")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Date (YYYY-MM-DD):");
                        ui.text_edit_singleline(&mut self.jump_date_str);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Time (HH:MM):");
                        ui.text_edit_singleline(&mut self.jump_time_str);
                    });
                    if let Some(err) = &self.jump_error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let jump_btn = egui::Button::new("Jump ▶")
                            .fill(ui.visuals().selection.bg_fill);
                        if ui.add(jump_btn).clicked() { do_jump = true; }
                        if ui.button("Cancel").clicked() { do_cancel = true; }
                    });
                });
            if do_jump {
                use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
                let parsed = (|| -> Result<i64, &'static str> {
                    let date = NaiveDate::parse_from_str(self.jump_date_str.trim(), "%Y-%m-%d")
                        .map_err(|_| "Invalid date (YYYY-MM-DD)")?;
                    let time = NaiveTime::parse_from_str(self.jump_time_str.trim(), "%H:%M")
                        .map_err(|_| "Invalid time (HH:MM)")?;
                    let naive = NaiveDateTime::new(date, time);
                    Local.from_local_datetime(&naive)
                        .single()
                        .map(|dt| dt.timestamp())
                        .ok_or("Ambiguous local time")
                })();
                match parsed {
                    Ok(target_s) => {
                        let (vs, ve) = self.current_visible_window(app);
                        let width = ve - vs;
                        let new_vs = target_s - width / 2;
                        self.set_visible_window(app, new_vs, new_vs + width);
                        self.jump_dialog_open = false;
                        self.jump_error = None;
                    }
                    Err(e) => self.jump_error = Some(e.to_string()),
                }
            }
            if do_cancel { self.jump_dialog_open = false; self.jump_error = None; }
        }

        let mut visible_range: Option<(i64, i64)> = None;
        // Each entry: (label, computed points). Multiple entries when a group has >1 energy file.
        let mut plot_series: Vec<(String, Vec<(i64, f64)>)> = Vec::new();
        let mut last_gantt_usable_width_px: f32 = 1.0;
        let mut last_gantt_gutter_width_px: f32 = GUTTER_WIDTH;

        let show_xy = app.show_xy_panel();
        let show_gantt = app.show_gantt();
        let sep_h = 8.0; // draggable handle height

        if show_gantt {
        let gantt_h = if show_xy {
            (ui.available_height() - self.xy_panel.panel_height - sep_h).max(100.0)
        } else {
            ui.available_height().max(100.0)
        };

        ui.allocate_ui(egui::vec2(ui.available_width(), gantt_h), |ui| {
            Frame::canvas(ui.style()).show(ui, |ui| {
                ui.visuals_mut().clip_rect_margin = 0.0;

                let fixed_timeline_y = ui.min_rect().top();
                let available_height = ui.max_rect().bottom() - ui.min_rect().bottom();

                ScrollArea::vertical().show(ui, |ui| {
                    let mut canvas = ui.available_rect_before_wrap();
                    canvas.max.y = f32::INFINITY;
                    let response =
                        ui.interact(canvas, ui.id().with("canvas"), Sense::click_and_drag());

                    let min_s = self.initial_start_s.unwrap();
                    let max_s = self.initial_end_s.unwrap();

                    let base_font = TextStyle::Body.resolve(ui.style());
                    let gutter_width = compute_gutter_width(
                        ui.ctx(),
                        &base_font,
                        &self.options,
                        app,
                    );

                    let info = Info {
                        ctx: ui.ctx().clone(),
                        canvas,
                        response,
                        painter: ui.painter_at(canvas),
                        text_height: ui.text_style_height(&TextStyle::Body),
                        start_s: min_s,
                        stop_s: max_s,
                        font_id: base_font,
                        gutter_width,
                    };

                    last_gantt_usable_width_px = info.usable_width();
                    self.last_canvas_usable_width_px = info.usable_width();
                    last_gantt_gutter_width_px = gutter_width;

                    interaction::interact_with_canvas(&mut self.options, &info.response, &info);

                    let where_to_put_timeline = info.painter.add(Shape::Noop);

                    let max_y = canvas::ui_canvas(
                        &mut self.options,
                        app,
                        &info,
                        fixed_timeline_y,
                        (min_s, max_s),
                        &mut self.job_details_windows,
                        gutter_width,
                    );

                    let mut used_rect = canvas;
                    used_rect.max.y = max_y;
                    used_rect.max.y = used_rect.max.y.max(used_rect.min.y + available_height);

                    let timeline_shapes = timeline::paint_timeline(
                        &info,
                        used_rect,
                        &self.options,
                        min_s,
                        gutter_width,
                    );
                    info.painter
                        .set(where_to_put_timeline, Shape::Vec(timeline_shapes));

                    let current_time_line = timeline::paint_current_time_line(
                        &info,
                        &self.options,
                        used_rect,
                        gutter_width,
                    );
                    info.painter.add(current_time_line);

                    ui.allocate_rect(used_rect, Sense::hover());

                    let visible_start_s = info.start_s
                        - ((self.options.sideways_pan_in_points / info.usable_width())
                            * self.options.canvas_width_s) as i64;
                    let visible_end_s = visible_start_s + self.options.canvas_width_s as i64;
                    visible_range = Some((visible_start_s, visible_end_s));

                    if show_xy {
                        plot_series = app.data.plot_series.clone();
                    }

                    if let (Some(start), Some(end)) = (
                        Local.timestamp_opt(visible_start_s, 0).earliest(),
                        Local.timestamp_opt(visible_end_s, 0).earliest(),
                    ) {
                        app.set_localdate(start, end);
                    }
                });
            });
        });
        } else if show_xy {
            // XY-only source: use saved visible range if available, else full data bounds.
            let default_vs = self.initial_start_s.unwrap_or_else(|| app.get_start_date().timestamp());
            let default_ve = self.initial_end_s.unwrap_or_else(|| app.get_end_date().timestamp());
            let (vs, ve) = self.current_xy_visible.unwrap_or((default_vs, default_ve));
            visible_range = Some((vs, ve));
            plot_series = app.data.plot_series.clone();
        }

        if show_xy {
            if show_gantt {
                // Draggable resize handle between Gantt and energy diagram.
                let (handle_rect, handle_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), sep_h),
                    egui::Sense::drag(),
                );
                if handle_resp.hovered() || handle_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                if handle_resp.dragged() {
                    // Drag up (negative delta) → XY panel gets taller; drag down → shorter.
                    self.xy_panel.panel_height = (self.xy_panel.panel_height - handle_resp.drag_delta().y)
                        .clamp(80.0, 700.0);
                }
                let stroke_color = if handle_resp.hovered() || handle_resp.dragged() {
                    ui.visuals().widgets.hovered.bg_stroke.color
                } else {
                    ui.visuals().widgets.noninteractive.bg_stroke.color
                };
                ui.painter().hline(
                    handle_rect.x_range(),
                    handle_rect.center().y,
                    egui::Stroke::new(if handle_resp.hovered() || handle_resp.dragged() { 2.0 } else { 1.0 }, stroke_color),
                );
            }

            if let Some((vs, ve)) = visible_range {
                let now_s = Local::now().timestamp();

                let y_axis_gutter = if show_gantt { last_gantt_gutter_width_px } else { 0.0 };
                let cluster_names_list = cluster_names(&app.data.strata_by_resource_id);
                let mut owners: Vec<String> = app.data.filtered_jobs.iter()
                    .map(|j| j.owner.clone()).collect();
                owners.sort();
                owners.dedup();
                if let Some((new_vs, new_ve)) = self.xy_panel.show(
                    ui, &plot_series, vs, ve, now_s, y_axis_gutter, show_gantt,
                    &cluster_names_list, &owners,
                    self.options.zoom_max_seconds, self.options.zoom_min_seconds,
                    self.options.scroll_zoom_sensitivity,
                ) {
                    if show_gantt {
                        // Gantt + energy: sync Gantt canvas to new range.
                        let new_width_s = (new_ve - new_vs).max(1) as f32;
                        self.options.canvas_width_s = new_width_s;
                        let start_s = self.initial_start_s.unwrap();
                        let canvas_w_px = last_gantt_usable_width_px.max(1.0);
                        let pan_px =
                            -(((new_vs - start_s) as f32) / self.options.canvas_width_s) * canvas_w_px;
                        self.options.sideways_pan_in_points = pan_px;
                        self.pending_navigation_refresh = true;
                    } else {
                        // XY-only: persist visible range directly.
                        self.current_xy_visible = Some((new_vs, new_ve));
                    }
                }
            }
        }

        self.job_details_windows.retain(|w| w.is_open());
        for window in self.job_details_windows.iter_mut() {
            window.ui(ui);
        }
    }
}

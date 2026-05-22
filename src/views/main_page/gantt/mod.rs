mod canvas;
mod interaction;
pub(crate) mod jobs;
mod labels;
mod theme;
mod timeline;
mod types;
mod energy_plot;
mod energy_estimate;
mod panels;

use crate::models::data_structure::resource::ResourceState;
use crate::models::utils::utils::{get_all_clusters, get_all_hosts, get_all_resources};
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
    AdminAction, AdminPanelState, CreatePresetPanel, CreateViewPanel, EditPresetPanel,
    EditViewPanel, EnergyPanelState, PresetPanelAction, ViewFormAction,
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
    let Ok(content) = std::fs::read_to_string("views.json") else { return fallback; };
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
        let _ = std::fs::write("views.json", json);
    }
}

use self::types::{gutter_stripes_total_w, Info, Options, GUTTER_WIDTH};


fn compute_gutter_width(
    ctx: &egui::Context,
    base_font: &FontId,
    options: &Options,
    app: &ApplicationContext,
    _all_clusters: &Vec<crate::models::data_structure::cluster::Cluster>,
) -> f32 {
    let n_total = options.levels.len();
    let stripes_w = gutter_stripes_total_w(n_total);

    let max_label = jobs::max_leaf_label(app, &options.levels, options.leaf_label_template.as_deref());
    let font_leaf = FontId::proportional((base_font.size).max(11.0));
    let label_text_w = ctx
        .fonts(|f| f.layout_no_wrap(max_label, font_leaf, Color32::BLACK).size().x);
    // 4px left pad + 4px right pad; minimum 40px label area so an empty view isn't invisible.
    let label_w = (label_text_w + 8.0).max(40.0);

    (label_w + stripes_w).min(650.0)
}

pub struct GanttChart {
    options: Options,
    job_details_windows: Vec<JobDetailsWindow>,
    initial_start_s: Option<i64>,
    initial_end_s: Option<i64>,
    last_data_source_index: Option<usize>,

    gantt_views: Vec<GanttView>,
    current_view_index: usize,
    leaf_info_presets: Vec<types::LeafInfoPreset>,

    last_canvas_usable_width_px: f32,
    pending_navigation_refresh: bool,
    delete_view_confirm: Option<usize>,
    delete_preset_confirm: Option<String>,

    energy: EnergyPanelState,
    admin: AdminPanelState,
    create_view: CreateViewPanel,
    create_preset: CreatePresetPanel,
    edit_view: EditViewPanel,
    edit_preset: EditPresetPanel,
}

impl Default for GanttChart {
    fn default() -> Self {
        let config = load_views_config();
        let gantt_cfg = crate::models::data_structure::gantt_config::GanttConfig::load();
        let mut options = Options::default();
        options.canvas_width_s = gantt_cfg.default_timespan_s as f32;
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
            last_data_source_index: None,
            last_canvas_usable_width_px: 1.0,
            pending_navigation_refresh: false,
            delete_view_confirm: None,
            delete_preset_confirm: None,
            gantt_views: config.views,
            current_view_index: 0,
            leaf_info_presets: config.leaf_info_presets,
            energy: EnergyPanelState::default(),
            admin: AdminPanelState::default(),
            create_view: CreateViewPanel::default(),
            create_preset: CreatePresetPanel::default(),
            edit_view: EditViewPanel::default(),
            edit_preset: EditPresetPanel::default(),
        }
    }
}

impl GanttChart {
    pub fn render_data_source_tabs(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        ui.add_space(4.0);

        let data_source_names = app.get_all_data_source_names();
        let current_index = app.import.current_data_source_index;

        ui.horizontal(|ui| {
            for (index, name) in data_source_names.iter().enumerate() {
                let is_active = index == current_index;
                let can_close = index != 0;

                let tab_color = if is_active {
                    ui.visuals().widgets.active.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                };

                let tab_text = if is_active {
                    egui::RichText::new(name).strong()
                } else {
                    egui::RichText::new(name)
                };

                let mut tab_button = egui::Button::new(tab_text).fill(tab_color).frame(true);
                if is_active {
                    tab_button = tab_button.stroke(egui::Stroke::new(
                        1.0,
                        ui.visuals().widgets.active.bg_stroke.color,
                    ));
                }

                ui.horizontal(|ui| {
                    if ui.add(tab_button).clicked() {
                        app.switch_to_data_source(index);
                    }
                    if can_close {
                        ui.add_space(-4.0);
                        let close_btn = egui::Button::new("×")
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, ui.visuals().text_color()));
                        if ui.add(close_btn).clicked() {
                            app.close_imported_data_source(index);
                        }
                    }
                });

                ui.add_space(4.0);
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
    }

    pub fn render_compact_toolbar(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        let ds_idx = app.import.current_data_source_index;
        if self.initial_start_s.is_none() || self.last_data_source_index != Some(ds_idx) {
            let start_s = app.get_start_date().timestamp();
            let end_s = app.get_end_date().timestamp();
            self.initial_start_s = Some(start_s);
            self.initial_end_s = Some(end_s);
            self.last_data_source_index = Some(ds_idx);
            let span_s = (end_s - start_s) as f32;
            if span_s > 0.0 && span_s < self.options.canvas_width_s {
                self.options.canvas_width_s = span_s.max(10.0);
                self.options.sideways_pan_in_points = 0.0;
            }
        }

        // "View" dropdown in the top toolbar — mirrors the tab row below
        let current_view_name = self.gantt_views
            .get(self.current_view_index)
            .map(|v| v.name.as_str())
            .unwrap_or("View");
        let is_admin = app.is_admin();
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
                    if is_admin {
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
                    }
                });
            }
            ui.separator();
            let create_btn = egui::Button::new("+ Create view");
            let create_btn = if is_admin { create_btn } else { create_btn.fill(egui::Color32::TRANSPARENT) };
            let resp = ui.add_enabled(is_admin, create_btn);
            let resp = if !is_admin { resp.on_hover_text("Admin access required") } else { resp };
            if resp.clicked() {
                self.create_view.reset_and_open();
                ui.close_menu();
            }
        });

        ui.menu_button(t!("app.gantt.settings.title"), |ui| {
            ui.set_max_height(500.0);
            self.options.job_color.ui(ui);
        });

        let admin_button = if is_admin {
            egui::Button::new("Admin")
        } else {
            egui::Button::new("Admin")
                .fill(Color32::from_gray(110))
                .stroke(egui::Stroke::new(1.0, Color32::from_gray(170)))
        };

        let response = ui.add(admin_button);
        if !is_admin {
            response.clone().on_hover_text(
                "Accès réservé aux administrateurs. Veuillez vous authentifier.",
            );
        }
        if response.clicked() && is_admin {
            self.admin.open = true;
        }

        ui.add_space(6.0);

        // Quick timeline navigation
        let base_font = TextStyle::Body.resolve(ui.style());
        let gutter_width = compute_gutter_width(
            ui.ctx(),
            &base_font,
            &self.options,
            app,
            &app.get_current_clusters(),
        );
        let fallback_usable_width = (ui.available_width() - gutter_width).max(1.0);
        let canvas_usable_width = if self.last_canvas_usable_width_px > 1.0 {
            self.last_canvas_usable_width_px
        } else {
            fallback_usable_width
        };
        let points_per_second = canvas_usable_width / self.options.canvas_width_s;
        let day_delta_s: i64 = 24 * 60 * 60;
        let week_delta_s: i64 = 7 * day_delta_s;

        ui.label(RichText::new("Nav:").text_style(TextStyle::Small));
        if ui.small_button("◀ 1w").clicked() {
            self.options.sideways_pan_in_points += week_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }
        if ui.small_button("◀ 1d").clicked() {
            self.options.sideways_pan_in_points += day_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }
        if ui.small_button("1d ▶").clicked() {
            self.options.sideways_pan_in_points -= day_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }
        if ui.small_button("1w ▶").clicked() {
            self.options.sideways_pan_in_points -= week_delta_s as f32 * points_per_second;
            self.options.zoom_to_relative_s_range = None;
            self.pending_navigation_refresh = true;
        }

        if ui.small_button(t!("app.gantt.now")).clicked() {
            self.options.zoom_to_relative_s_range = Some((
                ui.ctx().input(|i| i.time),
                (
                    0.,
                    (self.initial_end_s.unwrap() - self.initial_start_s.unwrap()) as f64,
                ),
            ));
            self.pending_navigation_refresh = true;
        }
    }
}

impl View for GanttChart {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        self.render_data_source_tabs(ui, app);

        let ds_idx = app.import.current_data_source_index;
        if self.initial_start_s.is_none() || self.last_data_source_index != Some(ds_idx) {
            let start_s = app.get_start_date().timestamp();
            let end_s = app.get_end_date().timestamp();
            self.initial_start_s = Some(start_s);
            self.initial_end_s = Some(end_s);
            self.last_data_source_index = Some(ds_idx);
            let span_s = (end_s - start_s) as f32;
            if span_s > 0.0 && span_s < self.options.canvas_width_s {
                self.options.canvas_width_s = span_s.max(10.0);
                self.options.sideways_pan_in_points = 0.0;
            }
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

        if app.import.current_data_source_index == 0 {
            app.data.all_jobs.retain(|j| j.id != 0);
        }

        let selected_cluster_names: Option<Vec<String>> = app
            .filters
            .selected_preset
            .as_ref()
            .and_then(|n| app.prefs.cluster_presets.iter().find(|p| p.name == *n))
            .map(|p| p.clusters.clone());

        let all_hosts = if let Some(cluster_names) = &selected_cluster_names {
            app.get_current_clusters()
                .iter()
                .filter(|c| cluster_names.contains(&c.name))
                .flat_map(|c| get_all_hosts(&vec![c.clone()]))
                .collect()
        } else {
            get_all_hosts(&app.get_current_clusters())
        };

        let all_clusters = if let Some(cluster_names) = &selected_cluster_names {
            cluster_names.clone()
        } else {
            get_all_clusters(&app.get_current_clusters())
        };

        let all_resources = if let Some(cluster_names) = &selected_cluster_names {
            app.get_current_clusters()
                .iter()
                .filter(|c| cluster_names.contains(&c.name))
                .flat_map(|c| get_all_resources(&vec![c.clone()]))
                .collect()
        } else {
            get_all_resources(&app.get_current_clusters())
        };

        if app.import.current_data_source_index == 0 {
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
                gantt_color: egui::Color32::TRANSPARENT,
                main_resource_state: ResourceState::Unknown,
                job_type: String::new(),
                job_types: Vec::new(),
                name: None,
                project: String::new(),
            });
        }

        // ── Dialog panels ─────────────────────────────────────────────────────
        let cluster_names: Vec<String> = app.get_current_clusters()
            .iter().map(|c| c.name.clone()).collect();
        let presets_snap = app.prefs.cluster_presets.clone();
        for action in self.admin.show(ui, &presets_snap, &cluster_names) {
            match action {
                AdminAction::SavePreset { preset, remove_old } => {
                    if let Some(old) = remove_old { app.remove_preset(&old); }
                    app.add_or_update_preset(preset);
                }
                AdminAction::DeletePreset(name) => app.remove_preset(&name),
            }
        }



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

        let mut visible_range: Option<(i64, i64)> = None;
        let mut energy_points: Vec<(i64, f64)> = Vec::new();
        let mut last_gantt_usable_width_px: f32 = 1.0;
        let mut last_gantt_gutter_width_px: f32 = GUTTER_WIDTH;

        let show_energy = app.show_energy_diagram();
        let show_gantt = app.show_gantt();
        let sep_h = 8.0; // draggable handle height

        if show_gantt {
        let gantt_h = if show_energy {
            (ui.available_height() - self.energy.panel_height - sep_h).max(100.0)
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
                        &app.get_current_clusters(),
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
                        &app.data.all_clusters,
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

                    if show_energy {
                        let energy_jobs: Vec<Job> = app
                            .data.filtered_jobs
                            .iter()
                            .filter(|job| {
                                let cluster_ok = match &self.energy.filter_cluster {
                                    Some(cluster) => job.clusters.iter().any(|c| c == cluster),
                                    None => true,
                                };
                                let owner_ok = match &self.energy.filter_owner {
                                    Some(owner) => &job.owner == owner,
                                    None => true,
                                };
                                let leaf_field = self.options.levels.last().map(|s| s.as_str()).unwrap_or("");
                                let view_ok = job.assigned_resources.iter().any(|&rid| {
                                    let Some(s) = app.data.strata_by_resource_id.get(&rid) else { return false; };
                                    let leaf_val = jobs::resolve_field(s, leaf_field, &app.data.strata_by_host);
                                    if leaf_val.starts_with("(no ") { return false; }
                                    match &self.options.resource_filter {
                                        None => true,
                                        Some(f) => {
                                            let actual = jobs::strata_field_value(s, &f.field).unwrap_or_default();
                                            let matches = actual.trim() == f.value.trim();
                                            f.exclude != matches
                                        }
                                    }
                                });
                                cluster_ok && owner_ok && view_ok
                            })
                            .cloned()
                            .collect();

                        energy_points = energy_estimate::compute_energy_points(
                            app.get_current_energy_series(),
                            &energy_jobs,
                            visible_start_s,
                            visible_end_s,
                        );
                    }

                    let start = Local.timestamp_opt(visible_start_s, 0).unwrap();
                    let end = Local.timestamp_opt(visible_end_s, 0).unwrap();
                    app.set_localdate(start, end);

                    if self.pending_navigation_refresh {
                        let never = *app.refresh.refresh_rate.lock().unwrap_or_else(|p| p.into_inner()) == u64::MAX;
                        if never {
                            self.pending_navigation_refresh = false;
                        } else {
                            let refreshing = *app
                                .refresh.is_refreshing
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            if !refreshing {
                                app.instant_update();
                                self.pending_navigation_refresh = false;
                            }
                        }
                    }
                });
            });
        });
        } else if show_energy {
            // Energy-only source: no Gantt rows, compute visible range from stored time bounds.
            let vs = self.initial_start_s.unwrap_or_else(|| app.get_start_date().timestamp());
            let ve = self.initial_end_s.unwrap_or_else(|| app.get_end_date().timestamp());
            visible_range = Some((vs, ve));
            energy_points = energy_estimate::compute_energy_points(
                app.get_current_energy_series(),
                &[],
                vs,
                ve,
            );
        }

        if show_energy {
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
                    // Drag up (negative delta) → energy gets taller; drag down → shorter.
                    self.energy.panel_height = (self.energy.panel_height - handle_resp.drag_delta().y)
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
                let cluster_names_energy = get_all_clusters(&app.get_current_clusters());
                let mut owners: Vec<String> = app.data.filtered_jobs.iter()
                    .map(|j| j.owner.clone()).collect();
                owners.sort();
                owners.dedup();
                if let Some((new_vs, new_ve)) = self.energy.show(
                    ui, &energy_points, vs, ve, now_s, y_axis_gutter, show_gantt,
                    &cluster_names_energy, &owners,
                ) {
                    let new_width_s = (new_ve - new_vs).max(1) as f32;
                    self.options.canvas_width_s = new_width_s;
                    let start_s = self.initial_start_s.unwrap();
                    let canvas_w_px = last_gantt_usable_width_px.max(1.0);
                    let pan_px =
                        -(((new_vs - start_s) as f32) / self.options.canvas_width_s) * canvas_w_px;
                    self.options.sideways_pan_in_points = pan_px;
                    self.pending_navigation_refresh = true;
                }
            }
        }

        self.job_details_windows.retain(|w| w.is_open());
        for window in self.job_details_windows.iter_mut() {
            window.ui(ui);
        }
    }
}

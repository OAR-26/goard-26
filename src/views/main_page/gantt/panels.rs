use eframe::egui;
use egui::{Color32, ScrollArea};
use std::collections::HashSet as StdHashSet;


use super::energy_plot;
use super::types::{self, ResourceFilter};
use super::GanttView;
use crate::models::data_structure::application_context::ClusterPreset;

// ---------------------------------------------------------------------------
// Known fields (shared by all form panels — view and preset editors)
// ---------------------------------------------------------------------------

pub(super) fn known_fields_sorted() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&str, &str)> = vec![
        ("site",                         "Site (derived from FQDN)"),
        ("cluster",                      "Cluster"),
        ("host",                         "Host / compute node"),
        ("type",                         "OAR resource type"),
        ("vlan",                         "VLAN ID"),
        ("disk",                         "Disk identifier"),
        ("disk_id",                      "Disk slot or 'node' for compute"),
        ("nodeset",                      "Nodeset"),
        ("subnet_address",               "Subnet address (full CIDR)"),
        ("subnet_prefix",                "Subnet prefix length"),
        ("slash_16",                     "Subnet /16 block"),
        ("slash_17",                     "Subnet /17 block"),
        ("slash_18",                     "Subnet /18 block"),
        ("slash_19",                     "Subnet /19 block"),
        ("slash_20",                     "Subnet /20 block"),
        ("slash_21",                     "Subnet /21 block"),
        ("slash_22",                     "Subnet /22 block"),
        ("state",                        "OAR resource state"),
        ("next_state",                   "Next OAR state"),
        ("production",                   "Production flag (YES/NO)"),
        ("network_address",              "Network address (FQDN)"),
        ("ip",                           "IP address"),
        ("comment",                      "Admin comment"),
        ("nodemodel",                    "Node model"),
        ("cputype",                      "CPU type"),
        ("cpufreq",                      "CPU frequency"),
        ("core_count",                   "Physical core count"),
        ("thread_count",                 "Thread count"),
        ("memnode",                      "Node memory (MB)"),
        ("memcore",                      "Memory per core (MB)"),
        ("gpu_model",                    "GPU model"),
        ("gpu_compute_capability",       "GPU compute capability"),
        ("chassis",                      "Chassis"),
        ("resource_id",                  "OAR resource ID"),
        ("besteffort",                   "Best-effort flag"),
        ("deploy",                       "Deploy flag"),
        ("drain",                        "Drain flag"),
        ("desktop_computing",            "Desktop computing flag"),
        ("state_num",                    "OAR state number"),
        ("rconsole",                     "Remote console"),
        ("cluster_priority",             "Cluster priority"),
        ("core",                         "Core index"),
        ("suspended_jobs",               "Suspended jobs"),
        ("eth_rate",                     "Ethernet rate (Gbps)"),
        ("gpudevice",                    "GPU device info"),
        ("cpuset",                       "CPU core set (ranges)"),
        ("available_upto",               "Available up to (timestamp)"),
        ("chunks",                       "OAR chunks"),
        ("cpu",                          "CPU ID"),
        ("cpu_count",                    "CPU count"),
        ("cpuarch",                      "CPU architecture"),
        ("cpucore",                      "Cores per CPU"),
        ("disk_reservation_count",       "Disk reservation count"),
        ("diskpath",                     "Disk path"),
        ("disktype",                     "Disk type"),
        ("eth_count",                    "Ethernet interface count"),
        ("eth_kavlan_count",             "Kavlan ethernet count"),
        ("exotic",                       "Exotic resource flag"),
        ("expiry_date",                  "Expiry date"),
        ("finaud_decision",              "Finaud decision"),
        ("gpu",                          "GPU ID"),
        ("gpu_compute_capability_major", "GPU compute capability (major)"),
        ("gpu_count",                    "GPU count"),
        ("gpu_mem",                      "GPU memory (MB)"),
        ("grub",                         "Grub flag"),
        ("ib",                           "InfiniBand flag"),
        ("ib_count",                     "InfiniBand interface count"),
        ("ib_rate",                      "InfiniBand rate (Gbps)"),
        ("last_available_upto",          "Last available up to (timestamp)"),
        ("last_job_date",                "Last job date"),
        ("maintenance",                  "Maintenance flag"),
        ("max_walltime",                 "Max walltime (s)"),
        ("mic",                          "MIC (Xeon Phi) flag"),
        ("myri",                         "Myrinet flag"),
        ("myri_count",                   "Myrinet interface count"),
        ("myri_rate",                    "Myrinet rate (Gbps)"),
        ("next_finaud_decision",         "Next finaud decision"),
        ("opa_count",                    "OmniPath interface count"),
        ("opa_rate",                     "OmniPath rate (Gbps)"),
        ("scheduler_priority",           "Scheduler priority"),
        ("switch",                       "Switch identifier"),
        ("virtual",                      "Virtualization flag"),
        ("wattmeter",                    "Wattmeter flag"),
    ];
    v.sort_unstable_by_key(|(f, _)| *f);
    v
}

// ---------------------------------------------------------------------------
// Energy panel
// ---------------------------------------------------------------------------

pub struct EnergyPanelState {
    pub filter_cluster: Option<String>,
    pub filter_owner: Option<String>,
    pub fit_to_figure: bool,
    pub y_bounds: Option<(f64, f64)>,
    pub panel_height: f32,
}

impl Default for EnergyPanelState {
    fn default() -> Self {
        Self {
            filter_cluster: None,
            filter_owner: None,
            fit_to_figure: true,
            y_bounds: None,
            panel_height: 270.0,
        }
    }
}

impl EnergyPanelState {
    /// Renders filter row + energy plot. Returns the new visible (start, end) range when the
    /// user drags inside the plot; the caller should pan the Gantt canvas accordingly.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        energy_series: &[(String, Vec<(i64, f64)>)],
        vs: i64,
        ve: i64,
        now_s: i64,
        y_axis_gutter: f32,
        show_gantt: bool,
        cluster_names: &[String],
        owner_names: &[String],
    ) -> Option<(i64, i64)> {
        ui.horizontal_wrapped(|ui| {
            if show_gantt {
                ui.label("Filtres énergie :");

                egui::ComboBox::from_id_salt("energy_filter_cluster")
                    .selected_text(
                        self.filter_cluster
                            .clone()
                            .unwrap_or_else(|| "Cluster: Tous".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filter_cluster, None, "Cluster: Tous");
                        for cluster in cluster_names {
                            ui.selectable_value(
                                &mut self.filter_cluster,
                                Some(cluster.clone()),
                                cluster,
                            );
                        }
                    });

                egui::ComboBox::from_id_salt("energy_filter_owner")
                    .selected_text(
                        self.filter_owner
                            .clone()
                            .unwrap_or_else(|| "Owner: Tous".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.filter_owner, None, "Owner: Tous");
                        for owner in owner_names {
                            ui.selectable_value(
                                &mut self.filter_owner,
                                Some(owner.clone()),
                                owner.clone(),
                            );
                        }
                    });

                if ui.small_button("Reset").clicked() {
                    self.filter_cluster = None;
                    self.filter_owner = None;
                }

                ui.separator();
            }

            let prev_fit = self.fit_to_figure;
            ui.checkbox(&mut self.fit_to_figure, "Ajuster à la figure");
            if self.fit_to_figure && !prev_fit {
                self.y_bounds = None;
            }
        });

        ui.add_space(4.0);

        let plot_h = if show_gantt {
            (self.panel_height - 60.0).max(60.0)
        } else {
            (ui.available_height() - 28.0).max(100.0)
        };

        let (maybe_new_range, new_y) = energy_plot::ui_energy_global(
            ui,
            energy_series,
            vs,
            ve,
            now_s,
            y_axis_gutter,
            plot_h,
            show_gantt,
            self.fit_to_figure,
            self.y_bounds,
        );
        if let Some(yb) = new_y {
            self.y_bounds = Some(yb);
        }

        maybe_new_range
    }
}

// ---------------------------------------------------------------------------
// Admin panel
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum AdminMode { New, Modify }

pub enum AdminAction {
    SavePreset {
        preset: ClusterPreset,
        remove_old: Option<String>,
    },
    DeletePreset(String),
}

pub struct AdminPanelState {
    pub open: bool,
    mode: Option<AdminMode>,
    selected_preset: Option<usize>,
    original_preset_name: Option<String>,
    preset_name: String,
    selected_clusters: StdHashSet<String>,
}

impl Default for AdminPanelState {
    fn default() -> Self {
        Self {
            open: false,
            mode: None,
            selected_preset: None,
            original_preset_name: None,
            preset_name: String::new(),
            selected_clusters: StdHashSet::new(),
        }
    }
}

impl AdminPanelState {
    /// Renders the admin window. Returns actions the caller should apply to `app`.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        presets: &[ClusterPreset],
        cluster_names: &[String],
    ) -> Vec<AdminAction> {
        let mut actions = Vec::new();
        if !self.open {
            return actions;
        }
        let mut open = self.open;
        egui::Window::new("Admin configuration")
            .open(&mut open)
            .default_width(300.0)
            .show(ui.ctx(), |ui| {
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.label("Cluster presets");
                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.mode == Some(AdminMode::New), "New Preset").clicked() {
                            self.mode = Some(AdminMode::New);
                            self.selected_preset = None;
                            self.original_preset_name = None;
                            self.preset_name.clear();
                            self.selected_clusters.clear();
                        }
                        if ui.selectable_label(self.mode == Some(AdminMode::Modify), "Modify Preset").clicked() {
                            self.mode = Some(AdminMode::Modify);
                            self.selected_preset = None;
                            self.original_preset_name = None;
                            self.preset_name.clear();
                            self.selected_clusters.clear();
                        }
                    });
                    ui.separator();

                    if self.mode == Some(AdminMode::Modify) {
                        egui::ComboBox::from_label("Select Preset")
                            .selected_text(
                                self.selected_preset
                                    .and_then(|i| presets.get(i))
                                    .map(|p| p.name.clone())
                                    .unwrap_or_else(|| "Select a preset".to_string()),
                            )
                            .show_ui(ui, |ui| {
                                for (i, preset) in presets.iter().enumerate() {
                                    if ui.selectable_value(&mut self.selected_preset, Some(i), &preset.name).clicked() {
                                        self.original_preset_name = Some(preset.name.clone());
                                        self.preset_name = preset.name.clone();
                                        self.selected_clusters = preset.clusters.iter().cloned().collect();
                                    }
                                }
                            });
                        ui.separator();
                    }

                    if self.mode == Some(AdminMode::New)
                        || (self.mode == Some(AdminMode::Modify) && self.selected_preset.is_some())
                    {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut self.preset_name);
                        ui.separator();
                        ui.label("Clusters to include");
                        ui.vertical(|ui| {
                            for name in cluster_names {
                                let mut checked = self.selected_clusters.contains(name);
                                if ui.checkbox(&mut checked, name).changed() {
                                    if checked { self.selected_clusters.insert(name.clone()); }
                                    else { self.selected_clusters.remove(name); }
                                }
                            }
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("Save").clicked() && !self.preset_name.trim().is_empty() {
                                let remove_old = if self.mode == Some(AdminMode::Modify)
                                    && self.original_preset_name.as_ref() != Some(&self.preset_name)
                                {
                                    self.original_preset_name.clone()
                                } else {
                                    None
                                };
                                actions.push(AdminAction::SavePreset {
                                    preset: ClusterPreset {
                                        name: self.preset_name.clone(),
                                        clusters: self.selected_clusters.iter().cloned().collect(),
                                    },
                                    remove_old,
                                });
                                self.mode = None;
                                self.selected_preset = None;
                                self.original_preset_name = None;
                                self.preset_name.clear();
                                self.selected_clusters.clear();
                            }
                            if self.mode == Some(AdminMode::Modify) && self.selected_preset.is_some() {
                                if ui.button("Delete").clicked() {
                                    if let Some(i) = self.selected_preset {
                                        if let Some(preset) = presets.get(i) {
                                            actions.push(AdminAction::DeletePreset(preset.name.clone()));
                                            self.mode = None;
                                            self.selected_preset = None;
                                            self.original_preset_name = None;
                                            self.preset_name.clear();
                                            self.selected_clusters.clear();
                                        }
                                    }
                                }
                            }
                        });
                    }
                });
            });
        self.open = open;
        actions
    }
}

// ---------------------------------------------------------------------------
// Create/Edit leaf-info preset panels
// ---------------------------------------------------------------------------

pub enum PresetPanelAction {
    Saved(types::LeafInfoPreset),
    Applied { idx: usize, name: String, fields: Vec<String> },
}

pub struct CreatePresetPanel {
    pub open: bool,
    pub name: String,
    pub fields: Vec<String>,
    pub error: Option<String>,
    field_search: String,
}

impl Default for CreatePresetPanel {
    fn default() -> Self {
        Self { open: false, name: String::new(), fields: Vec::new(), error: None, field_search: String::new() }
    }
}

impl CreatePresetPanel {
    pub fn reset_and_open(&mut self) {
        self.name.clear();
        self.fields.clear();
        self.error = None;
        self.field_search.clear();
        self.open = true;
    }

    /// Returns the saved preset when the user clicks "Save preset".
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PresetPanelAction> {
        if !self.open { return None; }
        let known = known_fields_sorted();
        let mut result = None;
        let mut still_open = true;
        egui::Window::new("Create info preset")
            .open(&mut still_open)
            .resizable(true)
            .default_width(420.0)
            .show(ui.ctx(), |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.set_min_width(380.0);
                    ui.label("Preset name:");
                    ui.text_edit_singleline(&mut self.name);
                    ui.add_space(6.0);
                    ui.label("Fields to show in tooltip:");
                    ui.text_edit_singleline(&mut self.field_search)
                        .on_hover_text("Filter fields");
                    ui.add_space(4.0);
                    let q = self.field_search.to_lowercase();
                    for (field, desc) in &known {
                        if !q.is_empty() && !field.to_lowercase().contains(&q) && !desc.to_lowercase().contains(&q) {
                            continue;
                        }
                        let mut checked = self.fields.iter().any(|f| f == field);
                        if ui.checkbox(&mut checked, *field).on_hover_text(*desc).changed() {
                            if checked { self.fields.push(field.to_string()); }
                            else { self.fields.retain(|f| f != field); }
                        }
                    }
                    ui.add_space(8.0);
                    if let Some(err) = &self.error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    if ui.button("Save preset").clicked() {
                        let name = self.name.trim().to_string();
                        if name.is_empty() {
                            self.error = Some("Name required.".into());
                        } else {
                            let id = name.to_lowercase().replace(' ', "_");
                            result = Some(PresetPanelAction::Saved(types::LeafInfoPreset {
                                id,
                                name,
                                fields: self.fields.clone(),
                            }));
                            self.open = false;
                        }
                    }
                });
            });
        if !still_open { self.open = false; }
        result
    }
}

pub struct EditPresetPanel {
    pub open: bool,
    pub idx: Option<usize>,
    pub name: String,
    pub fields: Vec<String>,
    pub error: Option<String>,
    field_search: String,
}

impl Default for EditPresetPanel {
    fn default() -> Self {
        Self { open: false, idx: None, name: String::new(), fields: Vec::new(), error: None, field_search: String::new() }
    }
}

impl EditPresetPanel {
    pub fn open_for(&mut self, idx: usize, preset: &types::LeafInfoPreset) {
        self.open = true;
        self.idx = Some(idx);
        self.name = preset.name.clone();
        self.fields = preset.fields.clone();
        self.error = None;
        self.field_search.clear();
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PresetPanelAction> {
        if !self.open { return None; }
        let known = known_fields_sorted();
        let mut result = None;
        let mut still_open = true;
        egui::Window::new("Edit preset")
            .open(&mut still_open)
            .resizable(true)
            .default_width(420.0)
            .show(ui.ctx(), |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.set_min_width(380.0);
                    ui.label("Preset name:");
                    ui.text_edit_singleline(&mut self.name);
                    ui.add_space(6.0);
                    ui.label("Fields to show in tooltip:");
                    ui.text_edit_singleline(&mut self.field_search)
                        .on_hover_text("Filter fields");
                    ui.add_space(4.0);
                    let q = self.field_search.to_lowercase();
                    for (field, desc) in &known {
                        if !q.is_empty() && !field.to_lowercase().contains(&q) && !desc.to_lowercase().contains(&q) {
                            continue;
                        }
                        let mut checked = self.fields.iter().any(|f| f == field);
                        if ui.checkbox(&mut checked, *field).on_hover_text(*desc).changed() {
                            if checked { self.fields.push(field.to_string()); }
                            else { self.fields.retain(|f| f != field); }
                        }
                    }
                    ui.add_space(8.0);
                    if let Some(err) = &self.error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            let name = self.name.trim().to_string();
                            if name.is_empty() {
                                self.error = Some("Name required.".into());
                            } else if let Some(idx) = self.idx {
                                result = Some(PresetPanelAction::Applied {
                                    idx,
                                    name,
                                    fields: self.fields.clone(),
                                });
                                self.open = false;
                            }
                        }
                        if ui.button("Cancel").clicked() { self.open = false; }
                    });
                });
            });
        if !still_open { self.open = false; }
        result
    }
}

// ---------------------------------------------------------------------------
// Create / Edit view panels
// ---------------------------------------------------------------------------

pub enum ViewFormAction {
    /// Create panel saved a new view.
    Created(GanttView),
    /// Edit panel applied changes to view at `idx`.
    Applied { idx: usize, view: GanttView },
    WantCreatePreset,
    WantEditPreset(usize),
    WantDeletePreset(String),
}

// Shared form state — used by both CreateViewPanel and EditViewPanel.
struct ViewFormState {
    name: String,
    levels: Vec<String>,
    template: String,
    summary_fields: Vec<String>,
    preset_id: Option<String>,
    sort_by_label: bool,
    filter_enabled: bool,
    filter_field: String,
    filter_value: String,
    filter_exclude: bool,
    error: Option<String>,
    field_search: String,
}

impl Default for ViewFormState {
    fn default() -> Self {
        Self {
            name: String::new(),
            levels: Vec::new(),
            template: String::new(),
            summary_fields: Vec::new(),
            preset_id: None,
            sort_by_label: false,
            filter_enabled: false,
            filter_field: String::new(),
            filter_value: String::new(),
            filter_exclude: false,
            error: None,
            field_search: String::new(),
        }
    }
}

fn show_view_form(
    form: &mut ViewFormState,
    ui: &mut egui::Ui,
    known: &[(&str, &str)],
    leaf_info_presets: &[types::LeafInfoPreset],
) -> Vec<ViewFormAction> {
    let mut actions = Vec::new();

    // Leaf-info preset dropdown
    ui.label("Leaf info preset (tooltip label & fields):");
    ui.horizontal(|ui| {
        let selected_name = form.preset_id.as_deref()
            .and_then(|id| leaf_info_presets.iter().find(|p| p.id == id))
            .map(|p| p.name.as_str())
            .unwrap_or("(none)");
        egui::ComboBox::from_id_salt("vf_preset")
            .selected_text(selected_name)
            .width(300.0)
            .show_ui(ui, |ui| {
                ui.set_min_width(300.0);
                let snap: Vec<(usize, String, String)> = leaf_info_presets.iter()
                    .enumerate().map(|(i, p)| (i, p.id.clone(), p.name.clone())).collect();
                ui.selectable_value(&mut form.preset_id, None, "(none)");
                let mut do_edit: Option<usize> = None;
                let mut do_delete: Option<String> = None;
                for (i, id, name) in &snap {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut form.preset_id, Some(id.clone()), name);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗑").on_hover_text("Delete").clicked() { do_delete = Some(id.clone()); }
                            if ui.small_button("✏").on_hover_text("Edit").clicked() { do_edit = Some(*i); }
                        });
                    });
                }
                if let Some(i) = do_edit { actions.push(ViewFormAction::WantEditPreset(i)); }
                if let Some(id) = do_delete { actions.push(ViewFormAction::WantDeletePreset(id)); }
            });
        if ui.button("+").on_hover_text("Create new preset").clicked() {
            actions.push(ViewFormAction::WantCreatePreset);
        }
    });
    ui.add_space(6.0);

    // Hierarchy levels — shown as horizontal colored boxes mirroring gutter stripes.
    ui.label("Hierarchy levels :");
    let mut remove_idx: Option<usize> = None;
    let mut swap: Option<(usize, usize)> = None;
    let n_levels = form.levels.len();
    let fill = Color32::from_rgb(252, 232, 80);
    let border = egui::Stroke::new(1.0, Color32::from_black_alpha(120));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, level) in form.levels.iter().enumerate() {
            let rounding = egui::Rounding {
                nw: if i == 0 { 4.0 } else { 0.0 },
                sw: if i == 0 { 4.0 } else { 0.0 },
                ne: if i + 1 == n_levels { 4.0 } else { 0.0 },
                se: if i + 1 == n_levels { 4.0 } else { 0.0 },
            };
            egui::Frame::none()
                .fill(fill)
                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                .rounding(rounding)
                .stroke(border)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if ui.add_enabled(
                            i > 0,
                            egui::Button::new("◀").frame(false).fill(Color32::TRANSPARENT),
                        ).clicked() { swap = Some((i - 1, i)); }

                        ui.label(level);

                        if ui.add_enabled(
                            i + 1 < n_levels,
                            egui::Button::new("▶").frame(false).fill(Color32::TRANSPARENT),
                        ).clicked() { swap = Some((i, i + 1)); }

                        if ui.small_button("🗑").clicked() { remove_idx = Some(i); }
                    });
                });
        }
    });
    if let Some(i) = remove_idx { form.levels.remove(i); }
    if let Some((a, b)) = swap { form.levels.swap(a, b); }
    ui.add_space(4.0);
    ui.label("Add field:");
    ui.text_edit_singleline(&mut form.field_search)
        .on_hover_text("Filter fields");
    ui.add_space(2.0);
    let fq = form.field_search.to_lowercase();
    ui.horizontal_wrapped(|ui| {
        for (field, desc) in known {
            if form.levels.iter().any(|l| l == field) { continue; }
            if !fq.is_empty() && !field.to_lowercase().contains(&fq) && !desc.to_lowercase().contains(&fq) {
                continue;
            }
            if ui.button(*field).on_hover_text(*desc).clicked() {
                form.levels.push(field.to_string());
            }
        }
    });
    ui.add_space(6.0);

    // Leaf label template
    ui.label("Leaf label template (e.g. {host|short}, {type}/{vlan}):");
    ui.text_edit_singleline(&mut form.template);
    ui.add_space(6.0);

    // Summary fields
    ui.label("Status bar fields (empty = last level):");
    let mut sf_remove: Option<usize> = None;
    for (i, f) in form.summary_fields.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(f);
            if ui.small_button("🗑").clicked() { sf_remove = Some(i); }
        });
    }
    if let Some(i) = sf_remove { form.summary_fields.remove(i); }
    ui.horizontal_wrapped(|ui| {
        for (field, _) in known {
            if form.summary_fields.iter().any(|f| f == field) { continue; }
            if ui.small_button(*field).clicked() {
                form.summary_fields.push(field.to_string());
            }
        }
    });
    ui.add_space(6.0);

    // Options
    ui.checkbox(&mut form.sort_by_label, "Sort by label (use when levels are opaque IDs)");
    ui.add_space(6.0);

    // Filter
    ui.checkbox(&mut form.filter_enabled, "Add resource filter");
    if form.filter_enabled {
        ui.indent("vf_filter_indent", |ui| {
            ui.horizontal(|ui| {
                ui.label("Field:");
                egui::ComboBox::from_id_salt("vf_filter_field")
                    .selected_text(if form.filter_field.is_empty() { "(select)" } else { &form.filter_field })
                    .show_ui(ui, |ui| {
                        for (f, desc) in known {
                            ui.selectable_value(&mut form.filter_field, f.to_string(), format!("{} - {}", f, desc));
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Value:");
                ui.text_edit_singleline(&mut form.filter_value);
            });
            ui.checkbox(&mut form.filter_exclude, "Exclude matching (denylist instead of allowlist)");
        });
    }
    ui.add_space(8.0);

    if let Some(err) = &form.error {
        ui.colored_label(egui::Color32::RED, err);
    }

    actions
}

fn validate_and_build_view(form: &mut ViewFormState) -> Option<GanttView> {
    if form.name.trim().is_empty() {
        form.error = Some("Name required.".into());
        return None;
    }
    if form.levels.is_empty() {
        form.error = Some("At least one level required.".into());
        return None;
    }
    if form.filter_enabled && (form.filter_field.is_empty() || form.filter_value.is_empty()) {
        form.error = Some("Filter requires field and value.".into());
        return None;
    }
    Some(GanttView {
        name: form.name.trim().to_string(),
        levels: form.levels.clone(),
        leaf_label_template: if form.template.trim().is_empty() { None } else { Some(form.template.trim().to_string()) },
        summary_fields: form.summary_fields.clone(),
        leaf_infos: form.preset_id.clone(),
        sort_by_label: form.sort_by_label,
        leaf_display_name: String::new(),
        leaf_hover_details: false,
        filter: if form.filter_enabled {
            Some(ResourceFilter {
                field: form.filter_field.clone(),
                value: form.filter_value.clone(),
                exclude: form.filter_exclude,
            })
        } else { None },
    })
}

// ── Create-view panel ─────────────────────────────────────────────────────────

pub struct CreateViewPanel {
    pub open: bool,
    form: ViewFormState,
}

impl Default for CreateViewPanel {
    fn default() -> Self { Self { open: false, form: ViewFormState::default() } }
}

impl CreateViewPanel {
    pub fn reset_and_open(&mut self) {
        self.form = ViewFormState::default();
        self.open = true;
    }

    pub fn set_preset_id(&mut self, id: Option<String>) {
        self.form.preset_id = id;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        leaf_info_presets: &[types::LeafInfoPreset],
    ) -> Vec<ViewFormAction> {
        if !self.open { return Vec::new(); }
        let known = known_fields_sorted();
        let mut actions = Vec::new();
        let mut still_open = true;
        egui::Window::new("Create view")
            .open(&mut still_open)
            .resizable(true)
            .default_width(520.0)
            .show(ui.ctx(), |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.set_min_width(480.0);
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.form.name);
                    ui.add_space(6.0);
                    actions.extend(show_view_form(&mut self.form, ui, &known, leaf_info_presets));
                    if ui.button("Save view").clicked() {
                        if let Some(view) = validate_and_build_view(&mut self.form) {
                            actions.push(ViewFormAction::Created(view));
                            self.open = false;
                        }
                    }
                });
            });
        if !still_open { self.open = false; }
        actions
    }
}

// ── Edit-view panel ───────────────────────────────────────────────────────────

pub struct EditViewPanel {
    pub open: bool,
    pub idx: Option<usize>,
    form: ViewFormState,
}

impl Default for EditViewPanel {
    fn default() -> Self { Self { open: false, idx: None, form: ViewFormState::default() } }
}

impl EditViewPanel {
    pub fn set_preset_id(&mut self, id: Option<String>) {
        self.form.preset_id = id;
    }

    pub fn open_for(&mut self, idx: usize, view: &GanttView) {
        self.open = true;
        self.idx = Some(idx);
        self.form = ViewFormState {
            name: view.name.clone(),
            levels: view.levels.clone(),
            template: view.leaf_label_template.clone().unwrap_or_default(),
            summary_fields: view.summary_fields.clone(),
            preset_id: view.leaf_infos.clone(),
            sort_by_label: view.sort_by_label,
            filter_enabled: view.filter.is_some(),
            filter_field: view.filter.as_ref().map(|f| f.field.clone()).unwrap_or_default(),
            filter_value: view.filter.as_ref().map(|f| f.value.clone()).unwrap_or_default(),
            filter_exclude: view.filter.as_ref().map(|f| f.exclude).unwrap_or(false),
            error: None,
            field_search: String::new(),
        };
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        leaf_info_presets: &[types::LeafInfoPreset],
    ) -> Vec<ViewFormAction> {
        if !self.open { return Vec::new(); }
        let known = known_fields_sorted();
        let mut actions = Vec::new();
        let mut still_open = true;
        egui::Window::new("Edit view")
            .open(&mut still_open)
            .resizable(true)
            .default_width(520.0)
            .show(ui.ctx(), |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.set_min_width(480.0);
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.form.name);
                    ui.add_space(6.0);
                    actions.extend(show_view_form(&mut self.form, ui, &known, leaf_info_presets));
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            if let Some(view) = validate_and_build_view(&mut self.form) {
                                if let Some(idx) = self.idx {
                                    actions.push(ViewFormAction::Applied { idx, view });
                                    self.open = false;
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() { self.open = false; }
                    });
                });
            });
        if !still_open { self.open = false; }
        actions
    }
}

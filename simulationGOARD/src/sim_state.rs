use chrono::{Local, TimeZone};
use eframe::egui;
use goard_core::models::data_structure::job_sorting::JobSortable;
use goard_core::models::data_structure::application_context::ApplicationContext;
use goard_core::models::data_structure::cluster::Cluster;
use goard_core::models::data_structure::job::Job;
use goard_core::models::data_structure::marker::GanttMarker;
use goard_core::models::data_structure::strata::Strata;
use goard_core::models::data_structure::tab_state_cache::{compute_file_hash, TabStateCache};
use goard_core::models::file_types::{FileTypeRegistry, VisualizationTarget};
use goard_core::views::main_page::gantt::GanttChart;

// ---------------------------------------------------------------------------
// Data types (moved from goard_core/src/models/data_structure/import_state.rs)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct DataSourceGroup {
    pub name: String,
    pub member_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct ImportedDataSource {
    pub name: String,
    pub file_path: Option<String>,
    pub file_hash: Option<String>,
    pub file_type_name: String,
    pub visualization_targets: Vec<VisualizationTarget>,
    pub jobs: Vec<Job>,
    pub clusters: Vec<Cluster>,
    pub strata: Vec<Strata>,
    pub raw_energy_series: Option<Vec<(i64, f64)>>,
    pub markers: Vec<GanttMarker>,
}

pub struct PendingImport {
    pub content: String,
    pub path: Option<String>,
    pub selected_type_name: Option<String>,
}

// ---------------------------------------------------------------------------
// SimState — owns all file/tab/group management
// ---------------------------------------------------------------------------

pub struct SimState {
    pub imported_data_sources: Vec<ImportedDataSource>,
    pub current_data_source_index: usize,
    pub request_file_import: bool,
    pub pending_import: Option<PendingImport>,
    pub groups: Vec<DataSourceGroup>,
    pub current_group_index: Option<usize>,
    pub pending_group_target: Option<usize>,
    tab_state_cache: TabStateCache,
}

impl Default for SimState {
    fn default() -> Self {
        Self {
            imported_data_sources: Vec::new(),
            current_data_source_index: 0,
            request_file_import: false,
            pending_import: None,
            groups: Vec::new(),
            current_group_index: None,
            pending_group_target: None,
            tab_state_cache: TabStateCache::load(),
        }
    }
}

impl SimState {
    // -----------------------------------------------------------------------
    // Core switch logic
    // -----------------------------------------------------------------------

    pub fn switch_to_data_source(&mut self, index: usize, app: &mut ApplicationContext) {
        self.current_group_index = None;
        if index == 0 {
            self.current_data_source_index = 0;
            app.data.all_jobs.clear();
            app.data.swap_all_jobs.clear();
            app.data.all_clusters.clear();
            app.data.swap_all_clusters.clear();
            app.data.strata_by_resource_id.clear();
            app.data.strata_by_host.clear();
            app.data.markers.clear();
        } else if self.imported_data_sources.get(index - 1).is_some() {
            self.current_data_source_index = index;
            app.data.dead_intervals.clear();
            app.data.standby_upto.clear();
            app.data.markers = self.imported_data_sources[index - 1].markers.clone();
            let strata = self.imported_data_sources[index - 1].strata.clone();
            app.data.strata_by_resource_id.clear();
            app.data.strata_by_host.clear();
            for r in &strata {
                if let Some(rid) = r.resource_id {
                    app.data.strata_by_resource_id.insert(rid, r.clone());
                }
                let host = r.host.as_deref().unwrap_or("").trim().to_string();
                let net = r.network_address.as_deref().unwrap_or("").trim().to_string();
                if !host.is_empty() {
                    app.data.strata_by_host.entry(host.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = host.split('.').next() {
                        if !short.is_empty() {
                            app.data.strata_by_host.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
                if !net.is_empty() {
                    app.data.strata_by_host.entry(net.clone()).or_insert_with(|| r.clone());
                    if let Some(short) = net.split('.').next() {
                        if !short.is_empty() {
                            app.data.strata_by_host.entry(short.to_string()).or_insert_with(|| r.clone());
                        }
                    }
                }
            }
            app.data.all_jobs = self.imported_data_sources[index - 1].jobs.clone();
            app.data.all_clusters = self.imported_data_sources[index - 1].clusters.clone();

            // Auto-fit time range to data span.
            let jobs = &self.imported_data_sources[index - 1].jobs;
            let mut min_time = i64::MAX;
            let mut max_time = i64::MIN;
            for job in jobs.iter().filter(|j| j.id != 0) {
                min_time = min_time.min(job.start_time).min(job.get_end_date());
                max_time = max_time.max(job.start_time).max(job.get_end_date());
            }
            if min_time == i64::MAX {
                if let Some(series) = self.imported_data_sources.get(index - 1)
                    .and_then(|ds| ds.raw_energy_series.as_deref())
                {
                    for &(ts, _) in series {
                        min_time = min_time.min(ts);
                        max_time = max_time.max(ts);
                    }
                }
                if min_time == i64::MAX {
                    for m in &app.data.markers {
                        min_time = min_time.min(m.timestamp_s);
                        max_time = max_time.max(m.timestamp_s);
                    }
                }
            }
            if min_time != i64::MAX && max_time != i64::MIN {
                let padding = (max_time - min_time) / 10;
                let start = Local.timestamp_opt(min_time - padding, 0).unwrap();
                let end = Local.timestamp_opt(max_time + padding, 0).unwrap();
                app.set_localdate(start, end);
            }
        } else {
            return;
        }
        self.sync_to_app(app);
        app.filter_jobs();
    }

    pub fn switch_to_group(&mut self, group_idx: usize, app: &mut ApplicationContext) {
        let Some(group) = self.groups.get(group_idx) else { return; };
        let members = group.member_indices.clone();
        let gantt_member = members.iter().copied().find(|&i| {
            self.imported_data_sources.get(i - 1)
                .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::Gantt))
                .unwrap_or(false)
        }).or_else(|| members.first().copied());
        if let Some(mi) = gantt_member {
            self.switch_to_data_source(mi, app);
        }
        self.current_group_index = Some(group_idx);
        self.sync_to_app(app);
    }

    pub fn close_group(&mut self, group_idx: usize, app: &mut ApplicationContext) {
        if group_idx >= self.groups.len() { return; }
        if self.current_group_index == Some(group_idx) {
            self.current_group_index = None;
            self.switch_to_data_source(0, app);
        } else if let Some(cur) = self.current_group_index {
            if cur > group_idx {
                self.current_group_index = Some(cur - 1);
            }
        }
        self.groups.remove(group_idx);
        self.sync_to_app(app);
    }

    pub fn delete_group(&mut self, group_idx: usize, app: &mut ApplicationContext) {
        if group_idx >= self.groups.len() { return; }
        let was_active = self.current_group_index == Some(group_idx);
        let target_after_delete: Option<(bool, usize)> = if was_active {
            if group_idx > 0 {
                Some((true, group_idx - 1))
            } else {
                let members_set: std::collections::HashSet<usize> =
                    self.groups[group_idx].member_indices.iter().copied().collect();
                let other_grouped: std::collections::HashSet<usize> = self.groups.iter()
                    .enumerate()
                    .filter(|(i, _)| *i != group_idx)
                    .flat_map(|(_, g)| g.member_indices.iter().copied())
                    .collect();
                let last_individual = (1..=self.imported_data_sources.len())
                    .filter(|i| !members_set.contains(i) && !other_grouped.contains(i))
                    .max();
                last_individual.map(|ds_idx| {
                    let shift = members_set.iter().filter(|&&m| m < ds_idx).count();
                    (false, ds_idx - shift)
                })
            }
        } else {
            None
        };
        if was_active {
            self.current_group_index = None;
            self.current_data_source_index = 0;
        }
        let mut members = self.groups[group_idx].member_indices.clone();
        members.sort_unstable_by(|a, b| b.cmp(a));
        self.groups.remove(group_idx);
        if let Some(cur) = self.current_group_index {
            if cur > group_idx {
                self.current_group_index = Some(cur - 1);
            }
        }
        for ds_idx in members {
            if ds_idx == 0 || ds_idx > self.imported_data_sources.len() { continue; }
            for g in &mut self.groups {
                for i in &mut g.member_indices {
                    if *i > ds_idx { *i -= 1; }
                }
            }
            self.imported_data_sources.remove(ds_idx - 1);
            match self.current_data_source_index.cmp(&ds_idx) {
                std::cmp::Ordering::Greater => self.current_data_source_index -= 1,
                std::cmp::Ordering::Equal   => self.current_data_source_index = 0,
                _ => {}
            }
        }
        if was_active {
            match target_after_delete {
                Some((true, gi)) if gi < self.groups.len() => {
                    self.switch_to_group(gi, app);
                }
                Some((false, ds)) if ds > 0 && ds <= self.imported_data_sources.len() => {
                    self.switch_to_data_source(ds, app);
                }
                _ => { self.sync_to_app(app); }
            }
        }
        app.filter_jobs();
    }

    pub fn remove_ds_from_group(&mut self, group_idx: usize, ds_idx: usize, app: &mut ApplicationContext) {
        if group_idx >= self.groups.len() { return; }
        self.groups[group_idx].member_indices.retain(|&i| i != ds_idx);
        let remaining_count = self.groups[group_idx].member_indices.len();
        for g in &mut self.groups {
            for i in &mut g.member_indices {
                if *i > ds_idx { *i -= 1; }
            }
        }
        self.close_imported_data_source(ds_idx, app);
        if remaining_count <= 1 {
            let was_active = self.current_group_index == Some(group_idx);
            let survivor = self.groups.get(group_idx)
                .and_then(|g| g.member_indices.first().copied());
            self.close_group(group_idx, app);
            if was_active {
                if let Some(m) = survivor {
                    self.switch_to_data_source(m, app);
                }
            }
        } else if self.current_group_index == Some(group_idx) {
            self.switch_to_group(group_idx, app);
        }
    }

    pub fn close_imported_data_source(&mut self, index: usize, app: &mut ApplicationContext) -> bool {
        if index == 0 { return false; }
        let actual_index = index - 1;
        if actual_index < self.imported_data_sources.len() {
            self.imported_data_sources.remove(actual_index);
            if self.current_data_source_index > index {
                self.current_data_source_index -= 1;
            } else if self.current_data_source_index == index {
                let target = if index > 1 { index - 1 } else { 0 };
                if target == 0 {
                    self.current_data_source_index = 0;
                    app.data.all_jobs.clear();
                    app.data.swap_all_jobs.clear();
                    app.data.all_clusters.clear();
                    app.data.swap_all_clusters.clear();
                    app.data.strata_by_resource_id.clear();
                    app.data.strata_by_host.clear();
                    app.data.markers.clear();
                    self.sync_to_app(app);
                } else {
                    self.switch_to_data_source(target, app);
                }
            }
            app.filter_jobs();
            return true;
        }
        false
    }

    // -----------------------------------------------------------------------
    // Import
    // -----------------------------------------------------------------------

    pub fn import_data_from_json(
        &mut self,
        app: &mut ApplicationContext,
        json_str: &str,
        file_path: Option<String>,
        type_name: Option<&str>,
    ) -> Result<(), String> {
        let registry = FileTypeRegistry::default();
        let file_type = if let Some(name) = type_name {
            registry.all_types().find(|t| t.name() == name)
                .ok_or_else(|| format!("Unknown file type: {}", name))?
        } else {
            registry.detect(json_str)
                .ok_or_else(|| "Unrecognized file format — no matching file type found.".to_string())?
        };
        let errors = file_type.validate(json_str);
        if !errors.is_empty() {
            let msg = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
            return Err(format!("Validation failed: {}", msg));
        }
        let parsed = file_type.parse(json_str)?;
        let base_name = file_path.as_ref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Data");
        let name = self.generate_unique_name(base_name);
        let file_hash = Some(compute_file_hash(json_str.as_bytes()));
        let file_path = file_path.map(|p| {
            std::fs::canonicalize(&p).ok()
                .and_then(|c| c.to_str().map(str::to_string))
                .unwrap_or(p)
        });
        self.imported_data_sources.push(ImportedDataSource {
            name,
            file_path,
            file_hash,
            file_type_name: file_type.name().to_string(),
            visualization_targets: file_type.visualization_targets().to_vec(),
            raw_energy_series: parsed.raw_energy_series,
            markers: parsed.markers,
            jobs: parsed.jobs,
            clusters: parsed.clusters,
            strata: parsed.resources,
        });
        let new_index = self.imported_data_sources.len();
        if let Some(target_ds) = self.pending_group_target.take() {
            let existing = self.groups.iter().position(|g| g.member_indices.contains(&target_ds));
            if let Some(gi) = existing {
                self.groups[gi].member_indices.push(new_index);
                self.switch_to_group(gi, app);
            } else {
                let gname = self.next_group_name();
                self.groups.push(DataSourceGroup {
                    name: gname,
                    member_indices: vec![target_ds, new_index],
                });
                let gi = self.groups.len() - 1;
                self.switch_to_group(gi, app);
            }
        } else {
            self.switch_to_data_source(new_index, app);
        }
        Ok(())
    }

    fn next_group_name(&self) -> String {
        format!("group{}", self.groups.len() + 1)
    }

    pub fn generate_unique_name(&self, base_name: &str) -> String {
        let mut name = base_name.to_string();
        let mut counter = 1;
        while self.imported_data_sources.iter().any(|ds| ds.name == name) {
            name = format!("{} ({})", base_name, counter);
            counter += 1;
        }
        name
    }

    // -----------------------------------------------------------------------
    // Sync flat fields into ApplicationContext after any switch
    // -----------------------------------------------------------------------

    pub fn sync_to_app(&self, app: &mut ApplicationContext) {
        let in_group = self.current_group_index.is_some();
        app.current_group_active = in_group;
        app.current_source_key = self.current_data_source_index;

        if in_group {
            let gi = self.current_group_index.unwrap();
            if let Some(g) = self.groups.get(gi) {
                app.show_energy = g.member_indices.iter().any(|&i| {
                    self.imported_data_sources.get(i - 1)
                        .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::EnergyDiagram))
                        .unwrap_or(false)
                });
                app.show_gantt_panel = g.member_indices.iter().any(|&i| {
                    self.imported_data_sources.get(i - 1)
                        .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::Gantt))
                        .unwrap_or(false)
                });
                app.current_energy_series = g.member_indices.iter().filter_map(|&i| {
                    let ds = self.imported_data_sources.get(i - 1)?;
                    if !ds.visualization_targets.contains(&VisualizationTarget::EnergyDiagram) { return None; }
                    Some((ds.name.clone(), ds.raw_energy_series.clone().unwrap_or_default()))
                }).collect();
                app.current_source_name = g.member_indices.iter().find_map(|&i| {
                    let ds = self.imported_data_sources.get(i - 1)?;
                    if ds.visualization_targets.contains(&VisualizationTarget::Gantt) {
                        Some(ds.name.clone())
                    } else { None }
                }).unwrap_or_default();
                app.current_file_path = None;
                app.current_file_hash = None;
                app.current_file_type_name = String::new();
                app.current_supports_hierarchy = true;
            }
        } else if self.current_data_source_index == 0 {
            app.show_energy = true;
            app.show_gantt_panel = true;
            app.current_energy_series = Vec::new();
            app.current_source_name = String::new();
            app.current_file_path = None;
            app.current_file_hash = None;
            app.current_file_type_name = String::new();
            app.current_supports_hierarchy = true;
        } else {
            let idx = self.current_data_source_index;
            if let Some(ds) = self.imported_data_sources.get(idx - 1) {
                app.show_energy = ds.visualization_targets.contains(&VisualizationTarget::EnergyDiagram);
                app.show_gantt_panel = ds.visualization_targets.contains(&VisualizationTarget::Gantt);
                app.current_energy_series = ds.raw_energy_series.as_ref()
                    .map(|s| vec![(ds.name.clone(), s.clone())])
                    .unwrap_or_default();
                app.current_source_name = ds.name.clone();
                app.current_file_path = ds.file_path.clone();
                app.current_file_hash = ds.file_hash.clone();
                app.current_file_type_name = ds.file_type_name.clone();
                let registry = FileTypeRegistry::default();
                app.current_supports_hierarchy = registry.find_by_name(&ds.file_type_name)
                    .map(|t| t.supports_hierarchy_controls())
                    .unwrap_or(true);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tab bar rendering (moved from GanttChart::render_data_source_tabs)
    // -----------------------------------------------------------------------

    pub fn render_tabs(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        ui.add_space(4.0);

        let current_index = self.current_data_source_index;
        let current_group = self.current_group_index;
        let stroke_color = ui.visuals().widgets.active.bg_stroke.color;
        let text_color = ui.visuals().text_color();
        let active_fill = ui.visuals().widgets.active.bg_fill;
        let inactive_fill = ui.visuals().widgets.inactive.bg_fill;

        let grouped_ds: std::collections::HashSet<usize> = self.groups.iter()
            .flat_map(|g| g.member_indices.iter().copied())
            .collect();
        let individual: Vec<(usize, String)> = self.imported_data_sources.iter()
            .enumerate()
            .filter(|(i, _)| !grouped_ds.contains(&(i + 1)))
            .map(|(i, ds)| (i + 1, ds.name.clone()))
            .collect();
        let groups_info: Vec<(usize, String, Vec<(usize, String)>)> = self.groups.iter()
            .enumerate()
            .map(|(gi, g)| {
                let members = g.member_indices.iter()
                    .filter_map(|&i| self.imported_data_sources.get(i - 1).map(|ds| (i, ds.name.clone())))
                    .collect();
                (gi, g.name.clone(), members)
            })
            .collect();

        let mut switch_to_ds: Option<usize> = None;
        let mut switch_to_group: Option<usize> = None;
        let mut close_ds: Option<usize> = None;
        let mut close_group_idx: Option<usize> = None;
        let mut group_target: Option<usize> = None;
        let mut remove_from_group: Option<(usize, usize)> = None;

        ui.horizontal(|ui| {
            for (ds_idx, name) in &individual {
                let is_active = *ds_idx == current_index && current_group.is_none();
                let fill = if is_active { active_fill } else { inactive_fill };
                let text = if is_active {
                    egui::RichText::new(name).strong()
                } else {
                    egui::RichText::new(name.as_str())
                };
                let mut btn = egui::Button::new(text).fill(fill).frame(true);
                if is_active {
                    btn = btn.stroke(egui::Stroke::new(1.0, stroke_color));
                }
                ui.horizontal(|ui| {
                    if ui.add(btn).clicked() {
                        switch_to_ds = Some(*ds_idx);
                    }
                    let plus = egui::Button::new("+")
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(1.0, text_color))
                        .min_size(egui::vec2(16.0, 16.0));
                    if ui.add(plus).on_hover_text("Group with another file").clicked() {
                        group_target = Some(*ds_idx);
                    }
                    ui.add_space(-4.0);
                    let close = egui::Button::new("×")
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(1.0, text_color));
                    if ui.add(close).clicked() {
                        close_ds = Some(*ds_idx);
                    }
                });
                ui.add_space(4.0);
            }

            for (gi, group_name, members) in &groups_info {
                let is_active = current_group == Some(*gi);
                let fill = if is_active { active_fill } else { inactive_fill };
                let popup_id = ui.make_persistent_id(("group_dd", *gi));
                let text = if is_active {
                    egui::RichText::new(group_name).strong()
                } else {
                    egui::RichText::new(group_name.as_str())
                };
                let mut name_btn = egui::Button::new(text).fill(fill).frame(true);
                if is_active {
                    name_btn = name_btn.stroke(egui::Stroke::new(1.0, stroke_color));
                }
                let name_resp = ui.add(name_btn);
                if name_resp.clicked() {
                    switch_to_group = Some(*gi);
                }
                let arrow_resp = ui.small_button("v");
                if arrow_resp.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                let tab_anchor = name_resp | arrow_resp;
                egui::popup::popup_below_widget(
                    ui, popup_id, &tab_anchor,
                    egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.set_min_width(180.0);
                        for (ds_idx, mn) in members {
                            ui.horizontal(|ui| {
                                ui.label(mn);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("🗑").on_hover_text("Delete file").clicked() {
                                        remove_from_group = Some((*gi, *ds_idx));
                                        ui.memory_mut(|m| m.close_popup());
                                    }
                                });
                            });
                        }
                    },
                );
                let first_member = members.first().map(|(i, _)| *i);
                let plus = egui::Button::new("+")
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, text_color))
                    .min_size(egui::vec2(16.0, 16.0));
                if ui.add(plus).on_hover_text("Add file to group").clicked() {
                    if let Some(m) = first_member {
                        group_target = Some(m);
                    }
                }
                let del = egui::Button::new("🗑")
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, text_color));
                if ui.add(del).on_hover_text("Delete group and all files").clicked() {
                    close_group_idx = Some(*gi);
                }
                ui.add_space(4.0);
            }
        });

        // Apply deferred actions.
        if let Some(i) = switch_to_ds               { self.switch_to_data_source(i, app); }
        if let Some(gi) = switch_to_group           { self.switch_to_group(gi, app); }
        if let Some(i) = close_ds                   { self.close_imported_data_source(i, app); }
        if let Some(gi) = close_group_idx           { self.delete_group(gi, app); }
        if let Some((gi, di)) = remove_from_group   { self.remove_ds_from_group(gi, di, app); }
        if let Some(target) = group_target {
            self.pending_group_target = Some(target);
            self.request_file_import = true;
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
    }

    // -----------------------------------------------------------------------
    // Tab state persistence (called on app exit)
    // -----------------------------------------------------------------------

    pub fn flush_all_tab_states(&mut self, gantt_view: &mut GanttChart) {
        let n = self.imported_data_sources.len();
        for tab_idx in 1..=n {
            if let Some(ds) = self.imported_data_sources.get(tab_idx - 1) {
                if let Some(hash) = ds.file_hash.as_deref() {
                    let is_active = tab_idx == self.current_data_source_index
                        && self.current_group_index.is_none();
                    gantt_view.persist_tab_state(
                        tab_idx,
                        is_active,
                        ds.file_path.as_deref(),
                        hash,
                    );
                }
            }
        }
    }
}

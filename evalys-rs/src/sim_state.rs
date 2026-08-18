use chrono::{Local, TimeZone};
use eframe::egui;
use ganttza::models::data_structure::job_sorting::JobSortable;
use ganttza::models::data_structure::application_context::ApplicationContext;
use ganttza::models::data_structure::job::Job;
use ganttza::models::data_structure::marker::GanttMarker;
use ganttza::models::data_structure::strata::Strata;
use crate::file_types::{FileTypeRegistry, VisualizationTarget};
use ganttza::views::main_page::gantt::{GanttChart, GanttViewSnapshot};

use crate::tab_state_cache::{compute_file_hash, TabStateCache, TabViewState};

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
// SimState — owns all file/tab/group management, including per-tab view
// memory (zoom/pan/selected view/energy panel state/hierarchy). goard_core
// has no notion of any of this — it just renders whatever GanttViewSnapshot
// it's handed.
// ---------------------------------------------------------------------------

pub struct SimState {
    pub imported_data_sources: Vec<ImportedDataSource>,
    pub current_data_source_index: usize,
    pub request_file_import: bool,
    pub pending_import: Option<PendingImport>,
    pub groups: Vec<DataSourceGroup>,
    pub current_group_index: Option<usize>,
    pub pending_group_target: Option<usize>,
    /// In-session view memory per data-source index (1-based).
    tab_snapshots: std::collections::HashMap<usize, GanttViewSnapshot>,
    /// Cross-session view memory, keyed by file identity.
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
            tab_snapshots: std::collections::HashMap::new(),
            tab_state_cache: TabStateCache::load(),
        }
    }
}

impl SimState {
    // -----------------------------------------------------------------------
    // View-snapshot bookkeeping
    // -----------------------------------------------------------------------

    /// Hierarchy levels this index should force (file types with a mandatory
    /// layout), or `None` to follow whatever Gantt View is selected.
    fn hierarchy_override_for(&self, index: usize) -> Option<Vec<String>> {
        if index == 0 { return None; }
        let ds = self.imported_data_sources.get(index - 1)?;
        FileTypeRegistry::default()
            .find_by_name(&ds.file_type_name)
            .and_then(|t| t.hierarchy_levels())
    }

    fn build_default_snapshot(&self, app: &ApplicationContext, hierarchy_override: Option<Vec<String>>) -> GanttViewSnapshot {
        let start_s = app.get_start_date().timestamp();
        let end_s = app.get_end_date().timestamp();
        let default_width = app.prefs.gantt_config.default_timespan_s as f32;
        let span_s = (end_s - start_s) as f32;
        let canvas_width_s = if span_s > 0.0 && span_s < default_width {
            span_s.max(10.0)
        } else {
            default_width
        };
        GanttViewSnapshot {
            initial_start_s: start_s,
            initial_end_s: end_s,
            canvas_width_s,
            sideways_pan_in_points: 0.0,
            rect_height: app.prefs.gantt_config.gantt_row_height,
            current_view_index: 0,
            xy_y_bounds: None,
            xy_fit_to_figure: true,
            xy_panel_height: app.prefs.gantt_config.xy_panel_height,
            xy_visible_range: None,
            hierarchy_override,
        }
    }

    /// Snapshot to apply for `index`: in-session memory, else the on-disk
    /// cache (by file identity), else a fresh default.
    fn snapshot_for(&mut self, index: usize, app: &ApplicationContext) -> GanttViewSnapshot {
        if let Some(s) = self.tab_snapshots.get(&index) {
            return s.clone();
        }
        let hierarchy_override = self.hierarchy_override_for(index);
        if index > 0 {
            if let Some(ds) = self.imported_data_sources.get(index - 1) {
                if let Some(hash) = ds.file_hash.as_deref() {
                    if let Some(saved) = self.tab_state_cache.lookup(ds.file_path.as_deref(), hash) {
                        return GanttViewSnapshot {
                            initial_start_s: app.get_start_date().timestamp(),
                            initial_end_s: app.get_end_date().timestamp(),
                            canvas_width_s: saved.canvas_width_s,
                            sideways_pan_in_points: saved.sideways_pan,
                            rect_height: saved.row_height,
                            current_view_index: saved.view_index,
                            xy_y_bounds: saved.energy_y_min.zip(saved.energy_y_max),
                            xy_fit_to_figure: saved.energy_fit,
                            xy_panel_height: saved.energy_panel_height,
                            xy_visible_range: None,
                            hierarchy_override,
                        };
                    }
                }
            }
        }
        self.build_default_snapshot(app, hierarchy_override)
    }

    fn persist_snapshot_to_disk(&mut self, file_path: Option<&str>, file_hash: &str, snap: &GanttViewSnapshot) {
        let state = TabViewState {
            canvas_width_s: snap.canvas_width_s,
            sideways_pan: snap.sideways_pan_in_points,
            row_height: snap.rect_height,
            view_index: snap.current_view_index,
            energy_y_min: snap.xy_y_bounds.map(|b| b.0),
            energy_y_max: snap.xy_y_bounds.map(|b| b.1),
            energy_fit: snap.xy_fit_to_figure,
            energy_panel_height: snap.xy_panel_height,
        };
        self.tab_state_cache.store(file_path, file_hash, state);
        self.tab_state_cache.save_to_disk();
    }

    /// Captures `gantt_view`'s live state into in-session memory for `index`,
    /// and to disk if it's backed by a file.
    fn capture_for(&mut self, index: usize, gantt_view: &GanttChart) {
        if index == 0 { return; }
        let snap = gantt_view.capture_view_snapshot();
        let file_id = self.imported_data_sources.get(index - 1)
            .and_then(|ds| ds.file_hash.clone().map(|hash| (ds.file_path.clone(), hash)));
        if let Some((path, hash)) = file_id {
            self.persist_snapshot_to_disk(path.as_deref(), &hash, &snap);
        }
        self.tab_snapshots.insert(index, snap);
    }

    /// Shifts in-session snapshot keys after a data source at `removed_idx`
    /// (1-based) was removed — mirrors how `imported_data_sources` shifts.
    fn shift_snapshots_after_removal(&mut self, removed_idx: usize) {
        let shifted: std::collections::HashMap<usize, GanttViewSnapshot> = std::mem::take(&mut self.tab_snapshots)
            .into_iter()
            .filter(|(k, _)| *k != removed_idx)
            .map(|(k, v)| if k > removed_idx { (k - 1, v) } else { (k, v) })
            .collect();
        self.tab_snapshots = shifted;
    }

    // -----------------------------------------------------------------------
    // Core switch logic
    // -----------------------------------------------------------------------

    pub fn switch_to_data_source(&mut self, index: usize, app: &mut ApplicationContext, gantt_view: &mut GanttChart) {
        let old_index = self.current_data_source_index;
        if old_index != index || self.current_group_index.is_some() {
            self.capture_for(old_index, gantt_view);
        }

        self.current_group_index = None;
        if index == 0 {
            self.current_data_source_index = 0;
            app.data.all_jobs.clear();
            app.data.cluster_resource_ids.clear();
            app.data.host_resource_ids.clear();
            app.data.cluster_hosts.clear();
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
            app.data.rebuild_cluster_index();

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

        let snapshot = self.snapshot_for(index, app);
        gantt_view.apply_view_snapshot(&snapshot);

        self.sync_to_app(app);
        app.filter_jobs();
    }

    pub fn switch_to_group(&mut self, group_idx: usize, app: &mut ApplicationContext, gantt_view: &mut GanttChart) {
        let Some(group) = self.groups.get(group_idx) else { return; };
        let members = group.member_indices.clone();
        let gantt_member = members.iter().copied().find(|&i| {
            self.imported_data_sources.get(i - 1)
                .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::Gantt))
                .unwrap_or(false)
        }).or_else(|| members.first().copied());
        if let Some(mi) = gantt_member {
            self.switch_to_data_source(mi, app, gantt_view); // clears current_group_index
        }
        self.current_group_index = Some(group_idx);
        self.sync_to_app(app);
    }

    pub fn close_group(&mut self, group_idx: usize, app: &mut ApplicationContext, gantt_view: &mut GanttChart) {
        if group_idx >= self.groups.len() { return; }
        if self.current_group_index == Some(group_idx) {
            self.current_group_index = None;
            self.switch_to_data_source(0, app, gantt_view);
        } else if let Some(cur) = self.current_group_index {
            if cur > group_idx {
                self.current_group_index = Some(cur - 1);
            }
        }
        self.groups.remove(group_idx);
        self.sync_to_app(app);
    }

    pub fn delete_group(&mut self, group_idx: usize, app: &mut ApplicationContext, gantt_view: &mut GanttChart) {
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
                if let Some(ds_idx) = last_individual {
                    let shift = members_set.iter().filter(|&&m| m < ds_idx).count();
                    Some((false, ds_idx - shift))
                } else if self.groups.len() > 1 {
                    // No individual files either — fall back to the next group,
                    // which shifts into slot 0 once this group is removed.
                    Some((true, 0))
                } else {
                    None
                }
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
            self.shift_snapshots_after_removal(ds_idx);
            match self.current_data_source_index.cmp(&ds_idx) {
                std::cmp::Ordering::Greater => self.current_data_source_index -= 1,
                std::cmp::Ordering::Equal   => self.current_data_source_index = 0,
                _ => {}
            }
        }
        if was_active {
            match target_after_delete {
                Some((true, gi)) if gi < self.groups.len() => {
                    self.switch_to_group(gi, app, gantt_view);
                }
                Some((false, ds)) if ds > 0 && ds <= self.imported_data_sources.len() => {
                    self.switch_to_data_source(ds, app, gantt_view);
                }
                _ => { self.sync_to_app(app); }
            }
        }
        app.filter_jobs();
    }

    pub fn remove_ds_from_group(&mut self, group_idx: usize, ds_idx: usize, app: &mut ApplicationContext, gantt_view: &mut GanttChart) {
        if group_idx >= self.groups.len() { return; }
        self.groups[group_idx].member_indices.retain(|&i| i != ds_idx);
        let remaining_count = self.groups[group_idx].member_indices.len();
        for g in &mut self.groups {
            for i in &mut g.member_indices {
                if *i > ds_idx { *i -= 1; }
            }
        }
        self.close_imported_data_source(ds_idx, app, gantt_view);
        if remaining_count <= 1 {
            let was_active = self.current_group_index == Some(group_idx);
            let survivor = self.groups.get(group_idx)
                .and_then(|g| g.member_indices.first().copied());
            self.close_group(group_idx, app, gantt_view);
            if was_active {
                if let Some(m) = survivor {
                    self.switch_to_data_source(m, app, gantt_view);
                }
            }
        } else if self.current_group_index == Some(group_idx) {
            self.switch_to_group(group_idx, app, gantt_view);
        }
    }

    pub fn close_imported_data_source(&mut self, index: usize, app: &mut ApplicationContext, gantt_view: &mut GanttChart) -> bool {
        if index == 0 { return false; }
        let actual_index = index - 1;
        if actual_index < self.imported_data_sources.len() {
            self.imported_data_sources.remove(actual_index);
            self.shift_snapshots_after_removal(index);
            // Other groups reference data sources by position — keep them in sync.
            for g in &mut self.groups {
                for i in &mut g.member_indices {
                    if *i > index { *i -= 1; }
                }
            }
            if self.current_data_source_index > index {
                self.current_data_source_index -= 1;
            } else if self.current_data_source_index == index {
                let target = if index > 1 {
                    // Left neighbor — unaffected by the removal, still valid.
                    index - 1
                } else if !self.imported_data_sources.is_empty() {
                    // No left neighbor — the tab that was to the right has
                    // shifted into this same 1-based slot after removal.
                    index
                } else {
                    0
                };
                if target == 0 {
                    self.current_data_source_index = 0;
                    app.data.all_jobs.clear();
                    app.data.cluster_resource_ids.clear();
                    app.data.host_resource_ids.clear();
                    app.data.cluster_hosts.clear();
                    app.data.strata_by_resource_id.clear();
                    app.data.strata_by_host.clear();
                    app.data.markers.clear();
                    self.sync_to_app(app);
                } else if let Some(gi) = self.groups.iter().position(|g| g.member_indices.contains(&target)) {
                    // The fallback slot belongs to a group — activate the group, not the bare file.
                    self.switch_to_group(gi, app, gantt_view);
                } else {
                    self.switch_to_data_source(target, app, gantt_view);
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
        gantt_view: &mut GanttChart,
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
            strata: parsed.resources,
        });
        let new_index = self.imported_data_sources.len();
        if let Some(target_ds) = self.pending_group_target.take() {
            let existing = self.groups.iter().position(|g| g.member_indices.contains(&target_ds));
            if let Some(gi) = existing {
                self.groups[gi].member_indices.push(new_index);
                self.switch_to_group(gi, app, gantt_view);
            } else {
                let gname = self.next_group_name();
                self.groups.push(DataSourceGroup {
                    name: gname,
                    member_indices: vec![target_ds, new_index],
                });
                let gi = self.groups.len() - 1;
                self.switch_to_group(gi, app, gantt_view);
            }
        } else {
            self.switch_to_data_source(new_index, app, gantt_view);
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

        if in_group {
            let gi = self.current_group_index.unwrap();
            if let Some(g) = self.groups.get(gi) {
                let has_gantt = g.member_indices.iter().any(|&i| {
                    self.imported_data_sources.get(i - 1)
                        .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::Gantt))
                        .unwrap_or(false)
                });
                let has_energy = g.member_indices.iter().any(|&i| {
                    self.imported_data_sources.get(i - 1)
                        .map(|ds| ds.visualization_targets.contains(&VisualizationTarget::EnergyDiagram))
                        .unwrap_or(false)
                });
                app.show_gantt_panel = has_gantt;
                app.show_xy_panel = has_energy;
                app.show_hierarchy_controls = true;

                // Gantt + energy files: estimated series from jobs first, then raw series.
                let mut series: Vec<(String, Vec<(i64, f64)>)> = Vec::new();
                if has_gantt && has_energy {
                    let (min_t, max_t) = job_time_range(&app.data.all_jobs);
                    if min_t < max_t {
                        let estimated = crate::energy_estimate::estimate_from_jobs(
                            &app.data.all_jobs, min_t, max_t, 10,
                            app.prefs.gantt_config.energy_watts_per_resource,
                        );
                        series.push(("Estimated".to_string(), estimated));
                    }
                }
                for &i in &g.member_indices {
                    let Some(ds) = self.imported_data_sources.get(i - 1) else { continue };
                    if !ds.visualization_targets.contains(&VisualizationTarget::EnergyDiagram) { continue; }
                    if let Some(raw) = &ds.raw_energy_series {
                        series.push((ds.name.clone(), crate::energy_estimate::series_from_raw(raw)));
                    }
                }
                app.data.plot_series = series;
            }
        } else if self.current_data_source_index == 0 {
            app.show_xy_panel = true;
            app.show_gantt_panel = true;
            app.show_hierarchy_controls = true;
            app.data.plot_series = Vec::new();
        } else {
            let idx = self.current_data_source_index;
            if let Some(ds) = self.imported_data_sources.get(idx - 1) {
                let has_energy = ds.visualization_targets.contains(&VisualizationTarget::EnergyDiagram);
                let has_gantt = ds.visualization_targets.contains(&VisualizationTarget::Gantt);
                app.show_xy_panel = has_energy;
                app.show_gantt_panel = has_gantt;
                app.data.plot_series = if let Some(raw) = &ds.raw_energy_series {
                    vec![(ds.name.clone(), crate::energy_estimate::series_from_raw(raw))]
                } else if has_gantt {
                    // Gantt-only file: estimate from jobs for the full data range.
                    let (min_t, max_t) = job_time_range(&app.data.all_jobs);
                    if min_t < max_t {
                        vec![("Estimated".to_string(), crate::energy_estimate::estimate_from_jobs(
                            &app.data.all_jobs, min_t, max_t, 10,
                            app.prefs.gantt_config.energy_watts_per_resource,
                        ))]
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                let registry = FileTypeRegistry::default();
                app.show_hierarchy_controls = registry.find_by_name(&ds.file_type_name)
                    .map(|t| t.supports_hierarchy_controls())
                    .unwrap_or(true);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tab bar rendering (moved from GanttChart::render_data_source_tabs)
    // -----------------------------------------------------------------------

    pub fn render_tabs(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext, gantt_view: &mut GanttChart) {
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
        if let Some(i) = switch_to_ds               { self.switch_to_data_source(i, app, gantt_view); }
        if let Some(gi) = switch_to_group           { self.switch_to_group(gi, app, gantt_view); }
        if let Some(i) = close_ds                   { self.close_imported_data_source(i, app, gantt_view); }
        if let Some(gi) = close_group_idx           { self.delete_group(gi, app, gantt_view); }
        if let Some((gi, di)) = remove_from_group   { self.remove_ds_from_group(gi, di, app, gantt_view); }
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
        // Capture the currently active individual tab's live state first.
        if self.current_group_index.is_none() && self.current_data_source_index > 0 {
            self.capture_for(self.current_data_source_index, gantt_view);
        }
        let snapshots = self.tab_snapshots.clone();
        for (idx, snap) in snapshots {
            if idx == 0 { continue; }
            let file_id = self.imported_data_sources.get(idx - 1)
                .and_then(|ds| ds.file_hash.clone().map(|hash| (ds.file_path.clone(), hash)));
            if let Some((path, hash)) = file_id {
                self.persist_snapshot_to_disk(path.as_deref(), &hash, &snap);
            }
        }
    }
}

fn job_time_range(jobs: &[ganttza::models::data_structure::job::Job]) -> (i64, i64) {
    jobs.iter().filter(|j| j.id != 0).fold((i64::MAX, i64::MIN), |(mn, mx), j| {
        (mn.min(j.start_time).min(j.get_end_date()),
         mx.max(j.start_time).max(j.get_end_date()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ganttza::models::data_structure::job::{Job, JobState};
    use ganttza::models::data_structure::resource::ResourceState;

    fn make_job(id: u32, start: i64, walltime: i64) -> Job {
        Job {
            id,
            owner: String::new(),
            state: JobState::Running,
            command: String::new(),
            walltime,
            message: None,
            queue: String::new(),
            assigned_resources: Vec::new(),
            scheduled_start: start,
            submission_time: 0,
            start_time: start,
            stop_time: start + walltime,
            exit_code: None,
            clusters: Vec::new(),
            hosts: Vec::new(),
            main_resource_state: ResourceState::Alive,
            job_type: String::new(),
            job_types: Vec::new(),
            name: None,
            project: String::new(),
        }
    }

    #[test]
    fn time_range_empty_jobs() {
        let (mn, mx) = job_time_range(&[]);
        assert_eq!(mn, i64::MAX);
        assert_eq!(mx, i64::MIN);
    }

    #[test]
    fn time_range_skips_job_0() {
        // job id=0 is the "all_resources" virtual row — must be ignored
        let jobs = vec![make_job(0, 0, 10000), make_job(1, 100, 50)];
        let (mn, mx) = job_time_range(&jobs);
        assert_eq!(mn, 100);
        assert_eq!(mx, 150);
    }

    #[test]
    fn time_range_multiple_jobs() {
        let jobs = vec![make_job(1, 100, 50), make_job(2, 200, 300), make_job(3, 50, 10)];
        let (mn, mx) = job_time_range(&jobs);
        assert_eq!(mn, 50);
        assert_eq!(mx, 500); // job 2 ends at 200+300
    }
}

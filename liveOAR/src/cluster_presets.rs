use eframe::egui;
use goard_core::models::data_structure::application_context::ApplicationContext;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClusterPreset {
    pub name: String,
    pub clusters: Vec<String>,
}

fn load_presets_from_file(path: &str) -> Vec<ClusterPreset> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_presets_to_file(presets: &[ClusterPreset], path: &str) {
    if let Ok(json) = serde_json::to_string(presets) {
        let _ = std::fs::write(path, json);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AdminMode {
    New,
    Modify,
}

/// Owns the list of cluster presets, the quick-select dropdown, and the
/// admin CRUD window. `goard_core` has no concept of presets — it only
/// understands a flat `selected_cluster_names` filter, which this state
/// sets directly on the `ApplicationContext`.
pub struct ClusterPresetState {
    presets: Vec<ClusterPreset>,
    selected_name: Option<String>,

    admin_open: bool,
    mode: Option<AdminMode>,
    selected_preset: Option<usize>,
    original_preset_name: Option<String>,
    preset_name: String,
    selected_clusters: HashSet<String>,
}

impl Default for ClusterPresetState {
    fn default() -> Self {
        Self {
            presets: load_presets_from_file("liveOAR/presets.json"),
            selected_name: None,
            admin_open: false,
            mode: None,
            selected_preset: None,
            original_preset_name: None,
            preset_name: String::new(),
            selected_clusters: HashSet::new(),
        }
    }
}

impl ClusterPresetState {
    pub fn open_admin(&mut self) {
        self.admin_open = true;
    }

    /// Quick-select dropdown: choosing a preset sets `selected_cluster_names`
    /// directly on the app context and re-filters.
    pub fn show_selector(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        egui::ComboBox::from_id_salt("cluster_preset_selector")
            .selected_text(
                self.selected_name
                    .clone()
                    .unwrap_or_else(|| "Cluster: All".to_string()),
            )
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut self.selected_name, None, "Cluster: All")
                    .clicked()
                {
                    app.filters.set_selected_cluster_names(None);
                    app.filter_jobs();
                }
                for preset in self.presets.clone() {
                    if ui
                        .selectable_value(&mut self.selected_name, Some(preset.name.clone()), &preset.name)
                        .clicked()
                    {
                        app.filters.set_selected_cluster_names(Some(preset.clusters.clone()));
                        app.filter_jobs();
                    }
                }
            });
    }

    /// Admin window: create/modify/delete presets. `cluster_names` is the
    /// full list of known clusters, used to populate the checkbox list.
    pub fn show_admin(&mut self, ui: &mut egui::Ui, cluster_names: &[String]) {
        if !self.admin_open {
            return;
        }
        let mut open = self.admin_open;
        let mut save_preset: Option<(ClusterPreset, Option<String>)> = None;
        let mut delete_preset: Option<String> = None;

        egui::Window::new("Cluster presets")
            .open(&mut open)
            .default_width(300.0)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
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
                                    .and_then(|i| self.presets.get(i))
                                    .map(|p| p.name.clone())
                                    .unwrap_or_else(|| "Select a preset".to_string()),
                            )
                            .show_ui(ui, |ui| {
                                for (i, preset) in self.presets.iter().enumerate() {
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
                                    if checked {
                                        self.selected_clusters.insert(name.clone());
                                    } else {
                                        self.selected_clusters.remove(name);
                                    }
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
                                save_preset = Some((
                                    ClusterPreset {
                                        name: self.preset_name.clone(),
                                        clusters: self.selected_clusters.iter().cloned().collect(),
                                    },
                                    remove_old,
                                ));
                                self.mode = None;
                                self.selected_preset = None;
                                self.original_preset_name = None;
                                self.preset_name.clear();
                                self.selected_clusters.clear();
                            }
                            if self.mode == Some(AdminMode::Modify) && self.selected_preset.is_some() {
                                if ui.button("Delete").clicked() {
                                    if let Some(i) = self.selected_preset {
                                        if let Some(preset) = self.presets.get(i) {
                                            delete_preset = Some(preset.name.clone());
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
        self.admin_open = open;

        if let Some((preset, remove_old)) = save_preset {
            if let Some(old) = remove_old {
                self.presets.retain(|p| p.name != old);
            }
            if let Some(existing) = self.presets.iter_mut().find(|p| p.name == preset.name) {
                *existing = preset;
            } else {
                self.presets.push(preset);
            }
            save_presets_to_file(&self.presets, "liveOAR/presets.json");
        }
        if let Some(name) = delete_preset {
            self.presets.retain(|p| p.name != name);
            save_presets_to_file(&self.presets, "liveOAR/presets.json");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_preset_serde_roundtrip() {
        let original = vec![
            ClusterPreset { name: "prod".to_string(), clusters: vec!["cluster-a".to_string(), "cluster-b".to_string()] },
            ClusterPreset { name: "dev".to_string(),  clusters: vec!["cluster-c".to_string()] },
        ];
        let json = serde_json::to_string(&original).unwrap();
        let decoded: Vec<ClusterPreset> = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn cluster_preset_empty_clusters_allowed() {
        let p = ClusterPreset { name: "empty".to_string(), clusters: Vec::new() };
        let json = serde_json::to_string(&p).unwrap();
        let decoded: ClusterPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.clusters.len(), 0);
    }

    #[test]
    fn load_presets_from_nonexistent_file_returns_empty() {
        let result = load_presets_from_file("/tmp/does_not_exist_goard_test.json");
        assert!(result.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let path = "/tmp/goard_test_presets.json";
        let presets = vec![
            ClusterPreset { name: "x".to_string(), clusters: vec!["c1".to_string()] },
        ];
        save_presets_to_file(&presets, path);
        let loaded = load_presets_from_file(path);
        assert_eq!(presets, loaded);
        let _ = std::fs::remove_file(path);
    }
}

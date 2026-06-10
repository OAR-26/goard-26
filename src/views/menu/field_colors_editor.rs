use std::collections::HashMap;
use eframe::egui;
use crate::models::data_structure::gantt_config::RgbColor;

pub struct FieldColorsEditor {
    pub open: bool,
    selected_field: Option<String>,
    draft: HashMap<String, HashMap<String, RgbColor>>,
    new_field_input: String,
    new_value_input: String,
}

impl Default for FieldColorsEditor {
    fn default() -> Self {
        Self {
            open: false,
            selected_field: None,
            draft: HashMap::new(),
            new_field_input: String::new(),
            new_value_input: String::new(),
        }
    }
}

fn rgb_to_arr(c: RgbColor) -> [f32; 3] {
    [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0]
}

fn arr_to_rgb(a: [f32; 3]) -> RgbColor {
    RgbColor(
        (a[0] * 255.0).round() as u8,
        (a[1] * 255.0).round() as u8,
        (a[2] * 255.0).round() as u8,
    )
}

impl FieldColorsEditor {
    pub fn open_with(&mut self, field_colors: &HashMap<String, HashMap<String, RgbColor>>) {
        self.draft = field_colors.clone();
        self.selected_field = None;
        self.new_field_input.clear();
        self.new_value_input.clear();
        self.open = true;
    }

    /// Returns `Some(field_colors)` when the user clicks Save, `None` otherwise.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<HashMap<String, HashMap<String, RgbColor>>> {
        if !self.open { return None; }

        let mut save   = false;
        let mut cancel = false;
        let mut still_open = true;

        egui::Window::new(t!("app.gantt.settings.field_colors_title"))
            .open(&mut still_open)
            .default_width(620.0)
            .min_width(480.0)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                let available = ui.available_height() - 40.0;
                ui.horizontal(|ui| {
                    // ── Left: field list ────────────────────────────────────
                    ui.vertical(|ui| {
                        ui.set_width(160.0);
                        ui.heading(t!("app.gantt.settings.field_colors_fields"));
                        ui.separator();

                        egui::ScrollArea::vertical()
                            .max_height(available)
                            .id_salt("fc_fields")
                            .show(ui, |ui| {
                                let mut fields: Vec<String> =
                                    self.draft.keys().cloned().collect();
                                fields.sort();
                                let mut delete_field: Option<String> = None;

                                for field in &fields {
                                    let sel = self.selected_field.as_deref() == Some(field.as_str());
                                    ui.horizontal(|ui| {
                                        if ui.selectable_label(sel, field).clicked() {
                                            self.selected_field = Some(field.clone());
                                            self.new_value_input.clear();
                                        }
                                        if ui.small_button("🗑").clicked() {
                                            delete_field = Some(field.clone());
                                            if self.selected_field.as_deref() == Some(field.as_str()) {
                                                self.selected_field = None;
                                            }
                                        }
                                    });
                                }
                                if let Some(f) = delete_field { self.draft.remove(&f); }
                            });

                        ui.separator();
                        ui.label(t!("app.gantt.settings.field_colors_new_field"));
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.new_field_input);
                            if ui.button(t!("app.gantt.settings.field_colors_add")).clicked() {
                                let name = self.new_field_input.trim().to_string();
                                if !name.is_empty() {
                                    self.draft.entry(name.clone()).or_default();
                                    self.selected_field = Some(name);
                                    self.new_field_input.clear();
                                }
                            }
                        });
                    });

                    ui.separator();

                    // ── Right: value→color rows ─────────────────────────────
                    ui.vertical(|ui| {
                        if let Some(field) = self.selected_field.clone() {
                            ui.heading(format!("\"{}\"", field));
                            ui.separator();

                            egui::ScrollArea::vertical()
                                .max_height(available - 48.0)
                                .id_salt("fc_values")
                                .show(ui, |ui| {
                                    if let Some(values) = self.draft.get_mut(&field) {
                                        let mut keys: Vec<String> =
                                            values.keys().cloned().collect();
                                        keys.sort();
                                        let mut delete_val: Option<String> = None;

                                        for key in &keys {
                                            if let Some(color) = values.get_mut(key) {
                                                ui.horizontal(|ui| {
                                                    let mut arr = rgb_to_arr(*color);
                                                    if ui.color_edit_button_rgb(&mut arr).changed() {
                                                        *color = arr_to_rgb(arr);
                                                    }
                                                    ui.label(key);
                                                    if ui.small_button("🗑").clicked() {
                                                        delete_val = Some(key.clone());
                                                    }
                                                });
                                            }
                                        }
                                        if let Some(v) = delete_val { values.remove(&v); }
                                    }
                                });

                            ui.separator();
                            ui.label(t!("app.gantt.settings.field_colors_new_value"));
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.new_value_input);
                                if ui.button(t!("app.gantt.settings.field_colors_add")).clicked() {
                                    let val = self.new_value_input.trim().to_string();
                                    if !val.is_empty() {
                                        if let Some(values) = self.draft.get_mut(&field) {
                                            values.entry(val).or_insert(RgbColor(180, 180, 180));
                                        }
                                        self.new_value_input.clear();
                                    }
                                }
                            });
                        } else {
                            ui.label(t!("app.gantt.settings.field_colors_select_hint"));
                        }
                    });
                });

                // ── Buttons ─────────────────────────────────────────────────
                ui.add_space(4.0);
                ui.separator();
                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new(t!("app.gantt.settings.field_colors_save"))
                        .fill(ui.visuals().selection.bg_fill);
                    if ui.add(save_btn).clicked() { save = true; }
                    if ui.button(t!("app.gantt.settings.field_colors_cancel")).clicked() { cancel = true; }
                });
            });

        if save {
            self.open = false;
            return Some(self.draft.clone());
        }
        if cancel || !still_open {
            self.open = false;
        }
        None
    }
}

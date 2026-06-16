use eframe::egui;
use egui::ScrollArea;
use crate::models::data_structure::{
    application_context::ApplicationContext,
    gantt_config::{GanttConfig, RgbColor},
};
use crate::views::components::gantt_job_color::{JobColor, JobColorEnum};
use crate::views::menu::field_colors_editor::FieldColorsEditor;

pub struct SettingsPanel {
    pub open: bool,
    draft: Option<(GanttConfig, JobColorEnum)>,
    field_colors_editor: FieldColorsEditor,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self { open: false, draft: None, field_colors_editor: FieldColorsEditor::default() }
    }
}

fn rgb_arr(c: RgbColor) -> [f32; 3] {
    [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0]
}

fn arr_rgb(a: [f32; 3]) -> RgbColor {
    RgbColor(
        (a[0] * 255.0).round() as u8,
        (a[1] * 255.0).round() as u8,
        (a[2] * 255.0).round() as u8,
    )
}

fn color_row(ui: &mut egui::Ui, label: &str, c: &mut RgbColor) {
    ui.horizontal(|ui| {
        let mut arr = rgb_arr(*c);
        if ui.color_edit_button_rgb(&mut arr).changed() {
            *c = arr_rgb(arr);
        }
        ui.label(label);
    });
}

impl SettingsPanel {
    pub fn show(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        self.show_with_extra(ui, app, |_ui, _app| {});
    }

    /// Same as `show`, but `extra` is rendered as its own section inside the
    /// Settings window, after core's sections — use it to let a binary add
    /// its own settings (e.g. an SSH host field) without core knowing what
    /// they're for. Returns true the frame core's Apply button is clicked,
    /// so the caller can persist whatever `extra` was editing at the same time.
    pub fn show_with_extra<F>(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext, extra: F) -> bool
    where
        F: FnOnce(&mut egui::Ui, &mut ApplicationContext),
    {
        if !self.open { return false; }

        // Initialise draft on open.
        if self.draft.is_none() {
            self.draft = Some((app.prefs.gantt_config.clone(), app.prefs.job_color.color.clone()));
        }

        let mut apply = false;
        let mut cancel = false;
        let mut still_open = self.open;

        egui::Window::new("⚙ Settings")
            .open(&mut still_open)
            .default_width(420.0)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                let Some((cfg, job_color_mode)) = self.draft.as_mut() else { return; };

                ScrollArea::vertical()
                    .max_height(ui.available_height() - 40.0)
                    .show(ui, |ui| {

                    // ── General ──────────────────────────────────────────────
                    ui.heading("General");
                    ui.checkbox(&mut cfg.standby_truncate_to_now,
                        "Truncate Absent intervals to now on Standby");
                    ui.checkbox(&mut cfg.besteffort_truncate_to_now,
                        "Truncate besteffort jobs to now");
                    ui.horizontal(|ui| {
                        ui.label("Min state duration (s):");
                        ui.add(egui::DragValue::new(&mut cfg.min_state_duration_s).range(0..=60));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Default timespan (s):");
                        ui.add(egui::DragValue::new(&mut cfg.default_timespan_s)
                            .range(60..=604800).suffix(" s"));
                    });
                    ui.add_space(6.0);

                    // ── Job colors ───────────────────────────────────────────
                    ui.heading("Job Colors");
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        ui.radio_value(job_color_mode, JobColorEnum::Random, "Random");
                        ui.radio_value(job_color_mode, JobColorEnum::ByField, "By field:");
                        if *job_color_mode == JobColorEnum::ByField {
                            ui.text_edit_singleline(&mut cfg.job_color_field);
                        }
                    });
                    if ui.button(t!("app.gantt.settings.field_colors_btn")).clicked() {
                        self.field_colors_editor.open_with(&cfg.field_colors);
                    }
                    ui.checkbox(&mut cfg.job_block_border, "Always show border around job blocks");
                    if cfg.job_block_border {
                        ui.horizontal(|ui| {
                            ui.label("Border width (px):");
                            ui.add(egui::Slider::new(&mut cfg.job_block_border_width, 0.5f32..=8.0f32).step_by(0.5));
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Random color minimum (0–255):");
                        ui.add(egui::Slider::new(&mut cfg.job_color_min, 0u8..=255u8));
                    });
                    ui.add_space(6.0);

                    // ── Gantt rows ───────────────────────────────────────────
                    ui.heading("Gantt Rows");
                    ui.horizontal(|ui| {
                        ui.label("Default height (px):");
                        ui.add(egui::DragValue::new(&mut cfg.gantt_row_height).range(4.0..=200.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Min height (px):");
                        ui.add(egui::DragValue::new(&mut cfg.gantt_row_height_min).range(2.0..=50.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Max height (px):");
                        ui.add(egui::DragValue::new(&mut cfg.gantt_row_height_max).range(20.0..=300.0));
                    });
                    ui.add_space(6.0);

                    // ── Energy ───────────────────────────────────────────────
                    ui.heading("Energy");
                    ui.horizontal(|ui| {
                        ui.label("Panel height (px):");
                        ui.add(egui::DragValue::new(&mut cfg.energy_panel_height).range(80.0..=700.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Watts per resource:");
                        ui.add(egui::DragValue::new(&mut cfg.energy_watts_per_resource).range(0.0..=10000.0));
                    });
                    color_row(ui, "Now-line color", &mut cfg.now_line_color);
                    ui.label("Series colors:");
                    let mut remove_idx: Option<usize> = None;
                    for (i, c) in cfg.energy_series_colors.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let mut arr = rgb_arr(*c);
                            if ui.color_edit_button_rgb(&mut arr).changed() { *c = arr_rgb(arr); }
                            ui.label(format!("Series {}", i + 1));
                            if ui.small_button("🗑").clicked() { remove_idx = Some(i); }
                        });
                    }
                    if let Some(i) = remove_idx { cfg.energy_series_colors.remove(i); }
                    if ui.small_button("+ Add color").clicked() {
                        cfg.energy_series_colors.push(RgbColor(100, 100, 200));
                    }
                    ui.add_space(6.0);

                    // ── Zoom ─────────────────────────────────────────────────
                    ui.heading("Zoom");
                    ui.horizontal(|ui| {
                        ui.label("Max (s):");
                        ui.add(egui::DragValue::new(&mut cfg.zoom_max_seconds)
                            .range(60.0..=2592000.0).suffix(" s"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Min (s):");
                        ui.add(egui::DragValue::new(&mut cfg.zoom_min_seconds)
                            .range(1.0..=3600.0).suffix(" s"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scroll sensitivity:");
                        ui.add(egui::DragValue::new(&mut cfg.scroll_zoom_sensitivity)
                            .range(0.0001..=0.05).speed(0.0001));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Drag sensitivity:");
                        ui.add(egui::DragValue::new(&mut cfg.drag_zoom_sensitivity)
                            .range(0.001..=0.1).speed(0.001));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Animation duration (s):");
                        ui.add(egui::DragValue::new(&mut cfg.zoom_animation_duration)
                            .range(0.1..=3.0).speed(0.05));
                    });
                    ui.add_space(6.0);

                    // ── Navigation ───────────────────────────────────────────
                    ui.heading("Navigation");
                    // Dynamic list — each entry = one ◀/▶ button pair, smallest to largest.
                    const UNITS: &[&str] = &["minute", "hour", "day", "week"];
                    let mut remove_step: Option<usize> = None;
                    let mut swap_step: Option<(usize, usize)> = None;
                    for (i, (n, unit)) in cfg.nav_steps.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("Step {}:", i + 1));
                            ui.add(egui::DragValue::new(n).range(1..=9999));
                            egui::ComboBox::from_id_salt(("nav_unit", i))
                                .selected_text(unit.as_str())
                                .show_ui(ui, |ui| {
                                    for &u in UNITS {
                                        ui.selectable_value(unit, u.to_string(), u);
                                    }
                                });
                            if i > 0 && ui.small_button("⬆").clicked() { swap_step = Some((i-1, i)); }
                            if ui.small_button("🗑").clicked() { remove_step = Some(i); }
                        });
                    }
                    if let Some((a, b)) = swap_step { cfg.nav_steps.swap(a, b); }
                    if let Some(i) = remove_step { cfg.nav_steps.remove(i); }
                    if ui.small_button("+ Add step").clicked() {
                        cfg.nav_steps.push((1, "day".to_string()));
                    }
                    ui.add_space(6.0);

                    // ── Layout ───────────────────────────────────────────────
                    ui.heading("Layout");
                    ui.horizontal(|ui| {
                        ui.label("Gutter max width (px):");
                        ui.add(egui::DragValue::new(&mut cfg.gutter_max_width).range(100.0..=1200.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Job label min width (px):");
                        ui.add(egui::DragValue::new(&mut cfg.job_label_min_width).range(0.0..=200.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Job bar label field:");
                        ui.text_edit_singleline(&mut cfg.job_label_field);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Hatch spacing (px):");
                        ui.add(egui::DragValue::new(&mut cfg.hatch_spacing).range(2.0..=50.0));
                    });
                    ui.add_space(6.0);

                    // ── State colors (dark) ───────────────────────────────────
                    ui.heading("State colors (dark mode)");
                    color_row(ui, "Absent",    &mut cfg.state_colors.absent);
                    color_row(ui, "Suspected", &mut cfg.state_colors.suspected);
                    color_row(ui, "Dead",      &mut cfg.state_colors.dead);
                    color_row(ui, "Standby",   &mut cfg.state_colors.standby);
                    ui.add_space(6.0);

                    // ── State colors (light) ──────────────────────────────────
                    ui.heading("State colors (light mode)");
                    color_row(ui, "Absent",    &mut cfg.state_colors_light.absent);
                    color_row(ui, "Suspected", &mut cfg.state_colors_light.suspected);
                    color_row(ui, "Dead",      &mut cfg.state_colors_light.dead);
                    color_row(ui, "Standby",   &mut cfg.state_colors_light.standby);
                    ui.add_space(6.0);

                    // ── App-specific settings ──────────────────────────────────
                    extra(ui, app);
                });

                ui.add_space(6.0);
                ui.separator();
                ui.horizontal(|ui| {
                    let apply_btn = egui::Button::new("✔  Apply")
                        .fill(ui.visuals().selection.bg_fill);
                    if ui.add(apply_btn).clicked() { apply = true; }
                    if ui.button("Cancel").clicked() { cancel = true; }
                });
            });

        // ── Field colors editor window ───────────────────────────────────────────
        if let Some(new_field_colors) = self.field_colors_editor.show(ui) {
            if let Some((cfg, _)) = self.draft.as_mut() {
                cfg.field_colors = new_field_colors;
            }
        }

        if apply {
            if let Some((cfg, mode)) = self.draft.take() {
                app.prefs.gantt_config = cfg;
                app.prefs.gantt_config.job_color_mode = match mode {
                    JobColorEnum::ByField => "field".to_string(),
                    JobColorEnum::Random  => "random".to_string(),
                };
                app.prefs.job_color = JobColor { color: mode };
                app.prefs.config_reload_requested = true;
                #[cfg(not(target_arch = "wasm32"))]
                app.prefs.gantt_config.save();
                // Re-init draft so panel stays usable after Apply.
                if still_open {
                    self.draft = Some((app.prefs.gantt_config.clone(), app.prefs.job_color.color.clone()));
                }
            }
        } else if cancel {
            self.draft = None;
            still_open = false;
        }

        if !still_open {
            self.draft = None;
            self.open = false;
        }

        apply
    }
}

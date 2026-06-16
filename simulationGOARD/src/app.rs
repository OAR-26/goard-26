use crate::sim_state::{PendingImport, SimState};
use goard_core::views::main_page::dashboard::Dashboard;
use goard_core::views::main_page::gantt::GanttChart;
use goard_core::views::menu::menu::Menu;
use goard_core::views::menu::tools::Tools;
use goard_core::views::view::View;
use goard_core::models::data_structure::application_context::ApplicationContext;
use eframe::egui::{self, CentralPanel, TopBottomPanel};

pub struct App {
    pub dashboard_view: Dashboard,
    pub gantt_view: GanttChart,
    pub menu: Menu,
    pub tools: Tools,
    pub application_context: ApplicationContext,
    pub sim_state: SimState,
}

impl App {
    pub fn new(import_entries: Vec<Vec<String>>) -> Self {
        let mut application_context = ApplicationContext::default();
        let mut sim_state = SimState::default();

        #[cfg(not(target_arch = "wasm32"))]
        for entry in &import_entries {
            let mut anchor_index: Option<usize> = None;
            for path in entry {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        if let Some(target) = anchor_index {
                            sim_state.pending_group_target = Some(target);
                        }
                        match sim_state.import_data_from_json(&mut application_context, &content, Some(path.clone()), None) {
                            Ok(()) => {
                                if anchor_index.is_none() {
                                    anchor_index = Some(sim_state.imported_data_sources.len());
                                }
                            }
                            Err(e) => eprintln!("Failed to import {}: {}", path, e),
                        }
                    }
                    Err(e) => eprintln!("Cannot read {}: {}", path, e),
                }
            }
        }

        App {
            dashboard_view: Dashboard::default(),
            gantt_view: GanttChart::default(),
            menu: Menu::default(),
            tools: Tools::default(),
            application_context,
            sim_state,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.menu.render_with_file_items(ui, &mut self.application_context, |ui, _app| {
                if ui.button("📁 Import File").clicked() {
                    goard_core::file_import::trigger_file_dialog();
                    ui.close_menu();
                }
            });
        });

        TopBottomPanel::top("tool_bar").show(ctx, |ui| {
            match self.application_context.view_type {
                goard_core::views::view::ViewType::Gantt => {
                    self.tools
                        .render_with_gantt(ui, &mut self.application_context, &mut self.gantt_view);
                }
                _ => {
                    self.tools.render(ui, &mut self.application_context);
                }
            }
        });

        TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            self.sim_state.render_tabs(ui, &mut self.application_context);
        });

        self.application_context.refresh_filters();

        // "+" group button requested a file dialog.
        if self.sim_state.request_file_import {
            self.sim_state.request_file_import = false;
            goard_core::file_import::trigger_file_dialog();
        }

        // File arrived from native picker — park it for type-selection dialog.
        if let Some((file_content, file_path)) = goard_core::file_import::take_file_content() {
            self.sim_state.pending_import = Some(PendingImport {
                content: file_content,
                path: file_path,
                selected_type_name: None,
            });
        }

        // Import type-selection dialog.
        if self.sim_state.pending_import.is_some() {
            use goard_core::models::file_types::FileTypeRegistry;

            let pending = self.sim_state.pending_import.as_ref().unwrap();
            let current_type = pending.selected_type_name.clone();
            let file_label = pending.path.as_deref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("(unknown file)")
                .to_string();

            let registry = FileTypeRegistry::default();
            let type_names: Vec<(String, String)> = registry
                .all_types()
                .map(|t| (t.name().to_string(), t.description().to_string()))
                .collect();

            let mut new_type: Option<Option<String>> = None;
            let mut do_import = false;
            let mut do_cancel = false;
            let mut import_error: Option<String> = None;

            egui::Window::new("Import File")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .min_width(300.0)
                .show(ctx, |ui| {
                    ui.label(format!("📄  {}", file_label));
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.label("File Type:");
                    ui.add_space(4.0);

                    if ui.radio(current_type.is_none(), "Auto Detect").clicked() {
                        new_type = Some(None);
                    }
                    for (name, desc) in &type_names {
                        let selected = current_type.as_deref() == Some(name.as_str());
                        let resp = ui.radio(selected, name.as_str()).on_hover_text(desc.as_str());
                        if resp.clicked() {
                            new_type = Some(Some(name.clone()));
                        }
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                        ui.add_space(8.0);
                        let import_btn = egui::Button::new("Import ▶").fill(ui.visuals().selection.bg_fill);
                        if ui.add(import_btn).clicked() {
                            do_import = true;
                        }
                    });
                });

            if let Some(t) = new_type {
                self.sim_state.pending_import.as_mut().unwrap().selected_type_name = t;
            }

            if do_cancel {
                self.sim_state.pending_import = None;
                self.sim_state.pending_group_target = None;
            } else if do_import {
                let pending = self.sim_state.pending_import.take().unwrap();
                let result = self.sim_state.import_data_from_json(
                    &mut self.application_context,
                    &pending.content,
                    pending.path,
                    pending.selected_type_name.as_deref(),
                );
                if let Err(e) = result {
                    import_error = Some(e);
                }
            }

            if let Some(err) = import_error {
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("Import failed: {}", err);
            }
        }

        TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .exact_height(18.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = ui;
                });
            });

        CentralPanel::default().show(ctx, |ui| match self.application_context.view_type {
            goard_core::views::view::ViewType::Dashboard => {
                self.dashboard_view.render(ui, &mut self.application_context);
            }
            goard_core::views::view::ViewType::Gantt => {
                self.gantt_view.render(ui, &mut self.application_context);
            }
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.sim_state.flush_all_tab_states(&mut self.gantt_view);
    }
}

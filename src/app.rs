use crate::models::utils::secret::Secret;
use crate::views::main_page::dashboard::Dashboard;
use crate::views::main_page::gantt::GanttChart;
use crate::views::menu::menu::Menu;
use crate::views::menu::tools::Tools;
use crate::views::view::View;
use crate::{
    models::data_structure::application_context::ApplicationContext,
    views::main_page::anthentification::Authentification,
};
use eframe::egui::{self, CentralPanel, TopBottomPanel};

pub struct App {
    pub dashboard_view: Dashboard,
    pub gantt_view: GanttChart,
    pub authentification_view: Authentification,
    pub menu: Menu,
    pub secret: Secret,
    pub tools: Tools,
    pub application_context: ApplicationContext,
}

impl App {
    pub fn new() -> Self {
        let app = App {
            secret: Secret::default(),
            dashboard_view: Dashboard::default(),
            gantt_view: GanttChart::default(),
            authentification_view: Authentification::default(),
            menu: Menu::default(),
            tools: Tools::default(),
            application_context: ApplicationContext::default(),
        };

        app
    }
    
    fn trigger_file_import(&mut self) {
        use crate::file_import;
        
        // Trigger the file dialog (works for both native and WASM)
        file_import::trigger_file_dialog();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.secret.update(ctx);
        self.secret.draw_snake_game(ctx);

        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.menu.render(ui, &mut self.application_context);
        });

        TopBottomPanel::top("tool_bar").show(ctx, |ui| {
            match self.application_context.view_type {
                crate::views::view::ViewType::Gantt => {
                    self.tools
                        .render_with_gantt(ui, &mut self.application_context, &mut self.gantt_view);
                }
                _ => {
                    self.tools.render(ui, &mut self.application_context);
                }
            }
        });

        // Check for updates
        self.application_context.check_data_update();
        
        // Handle file import request
        if self.application_context.import.request_file_import {
            self.application_context.import.request_file_import = false;
            self.trigger_file_import();
        }
        
        // File arrived from native picker → park it for type-selection dialog.
        {
            use crate::file_import;
            if let Some((file_content, file_path)) = file_import::take_file_content() {
                use crate::models::data_structure::import_state::PendingImport;
                self.application_context.import.pending_import = Some(PendingImport {
                    content: file_content,
                    path: file_path,
                    selected_type_name: None,
                });
            }
        }

        // Import type-selection dialog — shown while a file is pending.
        if self.application_context.import.pending_import.is_some() {
            use crate::models::file_types::FileTypeRegistry;

            let pending = self.application_context.import.pending_import.as_ref().unwrap();
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

            // Apply radio clicks collected above.
            if let Some(t) = new_type {
                self.application_context.import.pending_import.as_mut().unwrap().selected_type_name = t;
            }

            if do_cancel {
                self.application_context.import.pending_import = None;
                self.application_context.import.pending_group_target = None;
            } else if do_import {
                let pending = self.application_context.import.pending_import.take().unwrap();
                let result = self.application_context.import_data_from_json(
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

        // IMPORTANT: show the bottom panel BEFORE the central panel so it reserves space
        // instead of drawing on top of the Gantt rows.
        TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .exact_height(18.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if *self.application_context.refresh.is_refreshing.lock().unwrap() {
                        ui.add(egui::Spinner::new().size(12.0));
                        ui.label(egui::RichText::new(t!("app.refreshing")).small());
                    }
                });
            });

        CentralPanel::default().show(ctx, |ui| match self.application_context.view_type {
            crate::views::view::ViewType::Dashboard => {
                self.dashboard_view.render(ui, &mut self.application_context);
            }
            crate::views::view::ViewType::Gantt => {
                self.gantt_view.render(ui, &mut self.application_context);
            }
            crate::views::view::ViewType::Authentification => {
                self.authentification_view
                    .render(ui, &mut self.application_context);
            }
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

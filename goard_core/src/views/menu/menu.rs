use crate::{
    models::data_structure::{
        application_context::ApplicationContext, application_options::ApplicationOptions,
    },
    views::view::View,
};
use eframe::egui;

use super::settings_panel::SettingsPanel;
use super::options::Options;
use crate::views::view::ViewType;

pub struct Menu {
    options_pane: Options,
    settings_panel: SettingsPanel,
}

impl Default for Menu {
    fn default() -> Self {
        let application_options = ApplicationOptions::default();

        let options_pane = if std::path::Path::new("options.json").exists() {
            Options::load_from_file("options.json")
        } else {
            Options::new(application_options.clone())
        };

        Menu { options_pane, settings_panel: SettingsPanel::default() }
    }
}

impl Menu {
    /// Render the menu bar. `extra_file_items` is called inside the File dropdown
    /// before the login/logout and quit entries — use it to inject app-specific
    /// items (e.g. Import File, Live Data toggle).
    pub fn render_with_file_items<F>(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut ApplicationContext,
        extra_file_items: F,
    ) where
        F: FnOnce(&mut egui::Ui, &mut ApplicationContext),
    {
        if app.prefs.theme_toggle_requested {
            self.options_pane.toggle_theme(ui.ctx());
            app.prefs.theme_toggle_requested = false;
        }
        self.options_pane
            .apply_options(ui.ctx(), &mut app.prefs.font_size);

        ui.horizontal(|ui| {
            ui.menu_button(t!("app.menu.file"), |ui| {
                ui.set_max_width(200.0);
                ui.vertical(|ui| {
                    extra_file_items(ui, app);

                    ui.separator();
                    if ui.button(t!("app.menu.quit")).clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

            if ui.button(t!("app.menu.options")).clicked() {
                self.options_pane.open();
            }

            if ui.button("⚙ Settings").clicked() {
                self.settings_panel.open = true;
            }

            ui.menu_button("?", |ui| {
                match app.view_type {
                    ViewType::Gantt => {
                        ui.label(t!("app.gantt.help"));
                    }
                    ViewType::Dashboard => {
                        ui.label(t!("app.job_table.help"));
                    }
                }
            });

            self.options_pane.ui(ui, &mut app.prefs.font_size);
            self.settings_panel.show(ui, app);
        });
    }
}

impl View for Menu {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        self.render_with_file_items(ui, app, |_ui, _app| {});
    }
}

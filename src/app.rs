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
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.secret.update(ctx);
        self.secret.draw_snake_game(ctx);

        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.menu.render(ui, &mut self.application_context);
        });

        TopBottomPanel::top("tool_bar").show(ctx, |ui| {
            self.tools.render(ui, &mut self.application_context);
        });

        // Check for updates
        self.application_context.check_data_update();

        // IMPORTANT: show the bottom panel BEFORE the central panel so it reserves space
        // instead of drawing on top of the Gantt rows.
        TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .exact_height(18.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if *self.application_context.is_refreshing.lock().unwrap() {
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
        ctx.request_repaint();
    }
}

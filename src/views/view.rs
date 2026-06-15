use eframe::egui;

use crate::models::data_structure::application_context::ApplicationContext;

pub use crate::models::data_structure::view_type::ViewType;

pub trait View {
    fn render(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext);
}

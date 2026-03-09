use crate::models::data_structure::{
    application_context::ApplicationContext, filters::JobFilters, job::JobState,
};
use eframe::egui::{self, Grid, Stroke};
use strum::IntoEnumIterator;

/* `Filtering` manages the job filtering UI and functionality.
 * It provides a modal window where users can select various criteria
 * to filter jobs, including by owner, state, and resource (clusters/hosts).
 *
 * The component maintains temporary filter state until the user applies
 * the selected filters, at which point they are transferred to the main application.
 */
pub struct Filtering {
    open: bool,
    temp_filters: JobFilters,
}

impl Default for Filtering {
    fn default() -> Self {
        Filtering {
            open: false,
            temp_filters: JobFilters::default(),
        }
    }
}

impl Filtering {
    pub fn open(&mut self) {
        self.open = true;
    }

    /* Renders the filtering window and handles user interaction
     *
     * This window provides multiple filter categories (owners, states, clusters, hosts)
     * that users can select to narrow down the jobs displayed in the application.
     * Changes are only applied when the user clicks the Apply button.
     */
    pub fn ui(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        let mut open = self.open;
        // If the window is open, render the filters
        if self.open {
            egui::Window::new(t!("app.filter.page_title"))
                .collapsible(true)
                .movable(true)
                .open(&mut open)
                .default_size([600.0, 500.0])
                .show(ui.ctx(), |ui| {
                    ui.heading(t!("app.filter.title"));

                    ui.separator(); // Add a separator

                    egui::CollapsingHeader::new(t!("app.filter.owner"))
                        .default_open(false)
                        .show(ui, |ui| {
                            self.render_owners_selector(ui, app);
                        });
                    ui.add_space(10.0);

                    egui::CollapsingHeader::new(t!("app.filter.state"))
                        .default_open(false)
                        .show(ui, |ui| {
                            self.render_states_selector(ui);
                        });
                    ui.add_space(10.0);

                    ui.label("Cluster Presets");
                    ui.horizontal_wrapped(|ui| {
                        let none_selected = self.temp_filters.selected_preset.is_none();
                        let none_button = egui::Button::new("None")
                            .selected(none_selected)
                            .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color));
                        if ui.add(none_button).clicked() {
                            self.temp_filters.selected_preset = None;
                        }

                        for preset in &app.cluster_presets {
                            let is_selected = self
                                .temp_filters
                                .selected_preset
                                .as_deref()
                                .map(|name| name == preset.name)
                                .unwrap_or(false);

                            let preset_button = egui::Button::new(&preset.name)
                                .selected(is_selected)
                                .stroke(Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color));
                            if ui.add(preset_button).clicked() {
                                self.temp_filters.selected_preset = Some(preset.name.clone());
                            }
                        }
                    });

                    ui.add_space(20.0);

                    ui.horizontal(|ui| {
                        if ui.button(t!("app.filters.apply")).clicked() {
                            app.filters = JobFilters::copy(&self.temp_filters); // add the temporary filters to the app filters
                            app.filter_jobs(); // Filter the jobs
                            self.open = false; // Close the window
                        }
                        if ui.button(t!("app.filters.reset")).clicked() {
                            self.reset_filters(); // Reset the filters
                            app.filters = JobFilters::default();
                        }
                    });
                });
        }
        self.open = open;
    }

    pub fn reset_filters(&mut self) {
        self.temp_filters = JobFilters::default();
    }

    /* Renders the job owner selection grid
     *
     * This selector displays a grid of checkboxes for all unique job owners,
     * allowing the user to filter jobs by one or more owners.
     */
    fn render_owners_selector(&mut self, ui: &mut egui::Ui, app: &mut ApplicationContext) {
        let unique_owners = app.get_unique_owners();
        let mut selected_owners = self.temp_filters.owners.clone().unwrap_or_default();

        Grid::new("owners_grid")
            .num_columns(2)
            .spacing([10.0, 5.0])
            .show(ui, |ui| {
                for (i, owner) in unique_owners.iter().enumerate() {
                    let mut is_selected = selected_owners.contains(owner);
                    if ui.checkbox(&mut is_selected, owner).changed() {
                        if is_selected {
                            selected_owners.push(owner.clone());
                        } else {
                            selected_owners.retain(|o| o != owner);
                        }
                        self.temp_filters.set_owners(if selected_owners.is_empty() {
                            None
                        } else {
                            Some(selected_owners.clone())
                        });
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    }

    /*
     * Render the states selector
     * This selector is used to select the states of the jobs on which the jobs will be filtered
     */
    fn render_states_selector(&mut self, ui: &mut egui::Ui) {
        let mut selected_states = self.temp_filters.states.clone().unwrap_or_default();

        Grid::new("states_grid")
            .num_columns(2)
            .spacing([10.0, 5.0])
            .show(ui, |ui| {
                for (i, state) in JobState::iter().enumerate() {
                    let mut is_selected = selected_states.contains(&state);
                    if ui.checkbox(&mut is_selected, state.get_label()).changed() {
                        if is_selected {
                            selected_states.push(state);
                        } else {
                            selected_states.retain(|s| s != &state);
                        }
                        self.temp_filters.set_states(if selected_states.is_empty() {
                            None
                        } else {
                            Some(selected_states.clone())
                        });
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    }
}

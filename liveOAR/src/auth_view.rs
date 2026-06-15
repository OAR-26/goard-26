use eframe::egui::{self, RichText};

pub enum AuthOutcome {
    LoggedIn(String),
    ContinuedAsGuest,
}

pub struct AuthView {
    username: String,
    password: String,
    error_message: Option<String>,
}

impl Default for AuthView {
    fn default() -> Self {
        AuthView {
            username: String::new(),
            password: String::new(),
            error_message: None,
        }
    }
}

impl AuthView {
    /// Returns `Some(AuthOutcome)` when the user acts, `None` while idle.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<AuthOutcome> {
        let mut outcome: Option<AuthOutcome> = None;

        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.heading(RichText::new("Authentication").size(24.0));
            ui.add_space(20.0);

            egui::Frame::none()
                .fill(ui.visuals().window_fill())
                .rounding(8.0)
                .inner_margin(20.0)
                .show(ui, |ui| {
                    ui.set_width(300.0);

                    ui.label("Username");
                    ui.add_space(4.0);
                    ui.text_edit_singleline(&mut self.username);
                    ui.add_space(12.0);

                    ui.label("Password");
                    ui.add_space(4.0);
                    let pw = ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                    ui.add_space(20.0);

                    if let Some(err) = &self.error_message {
                        ui.colored_label(egui::Color32::RED, err);
                        ui.add_space(8.0);
                    }

                    let submit = ui.button("Login").clicked()
                        || (pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));

                    if submit {
                        if self.username == "admin" && self.password == "admin" {
                            outcome = Some(AuthOutcome::LoggedIn(self.username.clone()));
                            self.error_message = None;
                        } else {
                            self.error_message = Some("Invalid credentials".to_string());
                        }
                        self.password.clear();
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    if ui.button("Continue without logging in").clicked() {
                        outcome = Some(AuthOutcome::ContinuedAsGuest);
                    }
                });
        });

        outcome
    }
}

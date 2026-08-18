#[macro_use]
extern crate rust_i18n;
i18n!("src/i18n");

pub mod models;
pub mod views;

/// Localised window title for binary crates that launch the app.
pub fn window_title() -> String {
    t!("app.title").to_string()
}

/// Localised "refreshing data" text for the status bar spinner.
pub fn refreshing_text() -> String {
    t!("app.refreshing").to_string()
}

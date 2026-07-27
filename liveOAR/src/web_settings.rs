use goard_core::models::data_structure::application_options::ApplicationOptions;
use goard_core::models::data_structure::gantt_config::GanttConfig;

const KEY: &str = "goard_options";
const KEY_GANTT: &str = "goard_gantt_config";

pub fn load() -> ApplicationOptions {
    (|| -> Option<ApplicationOptions> {
        let storage = web_sys::window()?.local_storage().ok()??;
        let json = storage.get_item(KEY).ok()??;
        serde_json::from_str(&json).ok()
    })()
    .unwrap_or_default()
}

pub fn save(opts: &ApplicationOptions) {
    let _ = (|| -> Option<()> {
        let storage = web_sys::window()?.local_storage().ok()??;
        let json = serde_json::to_string(opts).ok()?;
        storage.set_item(KEY, &json).ok()
    })();
}

pub fn load_gantt_config() -> Option<GanttConfig> {
    let storage = web_sys::window()?.local_storage().ok()??;
    let json = storage.get_item(KEY_GANTT).ok()??;
    serde_json::from_str(&json).ok()
}

pub fn save_gantt_config(cfg: &GanttConfig) {
    let _ = (|| -> Option<()> {
        let storage = web_sys::window()?.local_storage().ok()??;
        let json = serde_json::to_string(cfg).ok()?;
        storage.set_item(KEY_GANTT, &json).ok()
    })();
}

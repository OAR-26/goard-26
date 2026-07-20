mod api_types;
mod app;
mod auth_view;
mod cluster_presets;
mod energy_estimate;
mod live_engine;
mod oar_fetch;
mod refresh_coordinator;
#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--serve") {
        let port: u16 = args.iter()
            .position(|a| a == "--port")
            .and_then(|i| args.get(i + 1))
            .and_then(|p| p.parse().ok())
            .unwrap_or(3030);
        let ssh_host = std::env::var("GOARD_SSH_HOST")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "grenoble.g5k".to_string());
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(server::run(ssh_host, port));
        return Ok(());
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        &goard_core::window_title(),
        options,
        Box::new(|_cc| Ok(Box::new(app::App::new()))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(app::App::new()))),
            )
            .await;

        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

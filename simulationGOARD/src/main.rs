mod app;
mod sim_state;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let import_entries: Vec<Vec<String>> = args.into_iter()
        .filter(|a| !a.starts_with("--"))
        .map(|a| {
            if a.contains('+') {
                a.split('+').map(|s| s.to_string()).filter(|s| !s.is_empty()).collect()
            } else {
                vec![a]
            }
        })
        .collect();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        &goard_core::window_title(),
        options,
        Box::new(move |_cc| Ok(Box::new(app::App::new(import_entries)))),
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
                Box::new(|_cc| Ok(Box::new(app::App::new(Vec::new())))),
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

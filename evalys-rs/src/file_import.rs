//! File import functionality for both WASM and native builds
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{FileReader, Event, HtmlInputElement, FileList};

#[cfg(not(target_arch = "wasm32"))]
use rfd;
#[cfg(not(target_arch = "wasm32"))]
use native_dialog::FileDialog;

// Global storage for file content and path
static mut FILE_CONTENT: Option<(String, Option<String>)> = None;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

/// Trigger a file dialog and store the file content globally
#[cfg(target_arch = "wasm32")]
pub fn trigger_file_dialog() {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    
    // Create a file input element
    let file_input = document.create_element("input").unwrap()
        .dyn_into::<HtmlInputElement>().unwrap();
    file_input.set_type("file");
    file_input.set_accept(".json");
    
    // Create a closure to handle file selection
    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |event: Event| {
        let target = event.target().unwrap();
        let input = target.dyn_into::<HtmlInputElement>().unwrap();
        
        if let Some(files) = input.files() {
            let files: &FileList = &files;
            if files.length() > 0 {
                let file = files.get(0).unwrap();
                
                // Create a file reader
                let reader = FileReader::new().unwrap();
                
                // Set up onload callback
                let onload_closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |event: Event| {
                    let target = event.target().unwrap();
                    let reader = target.dyn_into::<FileReader>().unwrap();
                    
                    match reader.result() {
                        Ok(result) => {
                            if let Some(result_str) = result.as_string() {
                                if !result_str.is_empty() {
                                    // Store the file content globally
                                    unsafe {
                                        FILE_CONTENT = Some((result_str, None)); // WASM doesn't easily provide file path
                                    }
                                    web_sys::console::log_1(&"File loaded successfully".into());
                                }
                            }
                        }
                        Err(_) => {
                            web_sys::console::log_1(&"Error reading file".into());
                        }
                    }
                }) as Box<dyn FnMut(_)>);
                
                reader.set_onload(Some(onload_closure.as_ref().unchecked_ref()));
                onload_closure.forget();
                
                // Read the file as text
                reader.read_as_text(&file).unwrap();
            }
        }
    }) as Box<dyn FnMut(_)>);
    
    file_input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget();
    
    // Trigger the file dialog
    file_input.click();
}

/// Trigger a file dialog and store the file content globally (native version)
#[cfg(not(target_arch = "wasm32"))]
pub fn trigger_file_dialog() {
    // Force the current thread to focus and wait for window to be ready
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // Try native-dialog first as it might have better window focus handling
    match FileDialog::new()
        .set_title("Import JSON File")
        .add_filter("JSON Files", &["json"])
        .show_open_single_file() 
    {
        Ok(Some(file)) => {
            match std::fs::read_to_string(&file) {
                Ok(content) => {
                    unsafe {
                        FILE_CONTENT = Some((content, Some(file.to_string_lossy().to_string())));
                    }
                    println!("File loaded successfully: {:?}", file);
                }
                Err(e) => {
                    eprintln!("Failed to read file: {}", e);
                }
            }
        }
        Ok(None) => {
            println!("No file selected");
        }
        Err(e) => {
            eprintln!("Error opening file dialog: {}", e);
            // Fallback to rfd if native-dialog fails
            fallback_to_rfd();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fallback_to_rfd() {
    let dialog = rfd::FileDialog::new()
        .add_filter("JSON Files", &["json"])
        .set_title("Import JSON File");
    
    if let Some(file) = dialog.pick_file() {
        match std::fs::read_to_string(&file) {
            Ok(content) => {
                unsafe {
                    FILE_CONTENT = Some((content, Some(file.to_string_lossy().to_string())));
                }
                println!("File loaded successfully: {:?}", file);
            }
            Err(e) => {
                eprintln!("Failed to read file: {}", e);
            }
        }
    }
}

/// Check if there's file content available and consume it
pub fn take_file_content() -> Option<(String, Option<String>)> {
    unsafe {
        FILE_CONTENT.take()
    }
}

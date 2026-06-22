//! Browser-facing wasm-bindgen wrapper for /tools/todo-organizer/.
use gizza_ai_todo_organizer_core::organize;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(text: &str, group_by: &str, numbered: &str, show_due: &str) -> Result<String, JsValue> {
    let group = if group_by.trim().is_empty() {
        "priority"
    } else {
        group_by
    };
    organize(text, group, truthy(numbered), truthy(show_due)).map_err(|e| JsValue::from_str(&e))
}

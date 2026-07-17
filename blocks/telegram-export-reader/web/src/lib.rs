//! Browser-facing wasm-bindgen wrapper for /tools/telegram-export-reader/.
//! The page passes every field as a string (in declared meta.toml order); this
//! parses the option fields and delegates to the pure core.
use gizza_ai_telegram_export_reader_core::{render, Options, Output};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    export: &str,
    output: &str,
    include_service_messages: &str,
    sender_filter: &str,
    max_messages: &str,
) -> Result<String, JsValue> {
    let sender = sender_filter.trim();
    let opts = Options {
        output: Output::parse(output),
        include_service: truthy(include_service_messages),
        sender_filter: if sender.is_empty() { None } else { Some(sender.to_string()) },
        max_messages: max_messages.trim().parse::<usize>().unwrap_or(0),
    };
    render(export, &opts).map_err(|e| JsValue::from_str(&e))
}

//! Browser-facing wasm-bindgen wrapper for /tools/mattermost-export-reader/.
//! The page passes every field as a string (in declared meta.toml order); this
//! parses the option fields and delegates to the pure core.
use gizza_ai_mattermost_export_reader_core::{render, Format, Options, Output};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    export: &str,
    output: &str,
    format: &str,
    channel: &str,
    user_filter: &str,
    since: &str,
    until: &str,
    include_direct_messages: &str,
    include_replies: &str,
    max_messages: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        output: Output::parse(output),
        format: Format::parse(format).map_err(|e| JsValue::from_str(&e))?,
        channel: opt(channel),
        user_filter: opt(user_filter),
        since: opt(since),
        until: opt(until),
        include_direct_messages: truthy(include_direct_messages),
        include_replies: truthy(include_replies),
        max_messages: max_messages.trim().parse::<usize>().unwrap_or(0),
    };
    render(export, &opts).map_err(|e| JsValue::from_str(&e))
}

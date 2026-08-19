//! Browser-facing wasm-bindgen wrapper for /tools/email-network-analyzer/.
//! The page passes every field as a string, so numbers/booleans are parsed here
//! and all validation stays in the shared core.
use wasm_bindgen::prelude::*;

/// Checkboxes arrive as `"true"`/`"false"`; treat every positive spelling as on.
fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

fn number(v: &str, what: &str, default: f64) -> Result<f64, JsValue> {
    let s = v.trim();
    if s.is_empty() {
        return Ok(default);
    }
    s.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{what} must be a whole number, got '{s}'")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    me: &str,
    nodes: &str,
    recipients: &str,
    direction: &str,
    top: &str,
    min_messages: &str,
    exclude: &str,
    self_loops: &str,
    since: &str,
    until: &str,
    format: &str,
) -> Result<String, JsValue> {
    let top = number(top, "top", 10.0)?;
    let min_messages = number(min_messages, "min_messages", 1.0)?;
    gizza_ai_email_network_analyzer_core::analyze(
        input,
        me,
        nodes,
        recipients,
        direction,
        top,
        min_messages,
        exclude,
        truthy(self_loops, false),
        since,
        until,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}

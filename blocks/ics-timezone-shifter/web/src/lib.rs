//! Browser-facing wasm-bindgen wrapper for /tools/ics-timezone-shifter/.
//! tool.js passes every page field as a raw string; this export parses the
//! boolean and delegates to the same pure core as the CLI/chat block.
use wasm_bindgen::prelude::*;

fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    from: &str,
    to: &str,
    mode: &str,
    write_as: &str,
    include_vtimezone: &str,
) -> Result<String, JsValue> {
    gizza_ai_ics_timezone_shifter_core::shift_str(
        input,
        from,
        to,
        mode,
        write_as,
        flag(include_vtimezone),
    )
    .map_err(|e| JsValue::from_str(&e))
}

//! Browser-facing wasm-bindgen wrapper for /tools/irc-log-parser/.
//! The page hands every field over as a string in `page/meta.toml` order, so the
//! numeric and boolean options are parsed here and empty fields fall back to the
//! descriptor defaults.
use wasm_bindgen::prelude::*;

/// Parse the page's record-limit field. A blank field means "no limit" (the
/// descriptor default); out-of-range values are left to the core so the user
/// gets the real error instead of a silent clamp.
fn num(raw: &str, default: i64) -> i64 {
    let t = raw.trim();
    if t.is_empty() {
        default
    } else {
        t.parse::<i64>().unwrap_or(default)
    }
}

/// A checkbox marshals as "true"/"false"; a blank value means the field was
/// never rendered, so fall back to the descriptor default.
fn flag(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    log: &str,
    format: &str,
    output: &str,
    date: &str,
    time_format: &str,
    include: &str,
    nicks: &str,
    channel: &str,
    strip_formatting: &str,
    include_raw: &str,
    limit: &str,
) -> Result<String, JsValue> {
    gizza_ai_irc_log_parser_core::run(
        log,
        format,
        output,
        date,
        time_format,
        include,
        nicks,
        channel,
        flag(strip_formatting, true),
        flag(include_raw, false),
        num(limit, 0),
    )
    .map_err(|e| JsValue::from_str(&e))
}

//! Browser-facing wasm-bindgen wrapper for /tools/json-log-formatter/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Format NDJSON `input` as an aligned log view / table / JSON / CSV.
///
/// The standalone tool page passes every field value as a string (and in the
/// `page/meta.toml` `[[input]]` order, which mirrors the descriptor's param
/// order), so the boolean/integer params arrive as strings and are parsed here:
/// - `level`:      `all` (blank) | `trace` | `debug` | `info` | `warn` | `error` | `fatal`.
/// - `field`:      dotted path the filter applies to; blank searches the whole record.
/// - `filter`:     the text to match; blank keeps everything.
/// - `match_mode`: `contains` (blank) | `exact`.
/// - `fields`:     comma-separated dotted paths to keep, in order; blank keeps all.
/// - `level_field` / `time_field` / `message_field`: explicit key names; blank auto-detects.
/// - `flatten`:    `"false"`/`"0"`/`"no"`/`"off"` → off; blank or anything else → on (the default).
/// - `on_invalid`: `skip` (blank) | `keep` | `error`.
/// - `limit`:      1–5000 (blank/unparseable → 0 → the core default of 200).
/// - `output`:     `pretty` (blank) | `table` | `json` | `csv`.
///
/// Throws a JS error string on empty input, an invalid enum value, or an
/// unparseable line when `on_invalid` is `error`.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    level: &str,
    field: &str,
    filter: &str,
    match_mode: &str,
    fields: &str,
    level_field: &str,
    time_field: &str,
    message_field: &str,
    flatten: &str,
    on_invalid: &str,
    limit: &str,
    output: &str,
) -> Result<String, JsValue> {
    // Default-on checkbox: only an explicit falsey string turns flattening off.
    let flatten = !matches!(
        flatten.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    );
    let limit = limit.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_json_log_formatter_core::format_logs(
        input,
        level,
        field,
        filter,
        match_mode,
        fields,
        level_field,
        time_field,
        message_field,
        flatten,
        on_invalid,
        limit,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

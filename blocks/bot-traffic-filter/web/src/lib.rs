//! Browser-facing wasm-bindgen wrapper for /tools/bot-traffic-filter/.
//! Compiled with wasm-pack for the standalone /tools/bot-traffic-filter/ page.
use wasm_bindgen::prelude::*;

/// Classify an access log / user-agent list and strip or report bot traffic.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean/integer params arrive as strings and are parsed here:
/// - `format`: `auto` (blank) | `combined` | `plain`.
/// - `output`: `report` (blank) | `table` | `json` | `csv` | `humans` | `bots`.
/// - `empty_is_bot`: `"true"`/`"1"`/`"yes"`/`"on"` → treat a missing/'-' UA as a
///   bot. A default-checked box sends `"true"`; unchecking sends `"false"`.
/// - `limit`: a count 1–10000 (blank/unparseable → 0 → the core default of 500).
///
/// Throws a JS error string on an invalid `format`/`output` or empty input.
#[wasm_bindgen]
pub fn run(
    input: &str,
    format: &str,
    output: &str,
    empty_is_bot: &str,
    limit: &str,
) -> Result<String, JsValue> {
    let empty_is_bot = matches!(
        empty_is_bot.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let limit = limit.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_bot_traffic_filter_core::filter(input, format, output, empty_is_bot, limit)
        .map_err(|e| JsValue::from_str(&e))
}

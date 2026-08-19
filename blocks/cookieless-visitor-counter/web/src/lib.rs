//! Browser-facing wasm-bindgen wrapper for /tools/cookieless-visitor-counter/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Count unique visitors in an access log with the daily-salted-hash method.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean/integer params arrive as strings and are parsed here:
/// - `format`: `auto` (blank) | `combined` | `common` | `json` | `csv`.
/// - `identity`: `ip_ua` (blank) | `ip` | `network_ua`.
/// - `period`: `day` (blank) | `hour` | `month` | `total`.
/// - `salt`: any secret string; blank uses the built-in reproducible salt.
/// - `exclude_bots`: `"true"`/`"1"`/`"yes"`/`"on"` → drop crawler hits. A
///   default-checked box sends `"true"`; unchecking sends `"false"`.
/// - `hash_length`: 6–64 (blank/unparseable → 0 → the core default of 12).
/// - `output`: `report` (blank) | `table` | `json` | `csv` | `ids`.
///
/// Throws a JS error string on an unknown enum value, an out-of-range
/// hash_length, an empty log, or a log with no readable requests.
#[wasm_bindgen]
pub fn run(
    input: &str,
    format: &str,
    identity: &str,
    period: &str,
    salt: &str,
    exclude_bots: &str,
    hash_length: &str,
    output: &str,
) -> Result<String, JsValue> {
    let exclude_bots = matches!(
        exclude_bots.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let hash_length = hash_length.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_cookieless_visitor_counter_core::count(
        input,
        format,
        identity,
        period,
        salt,
        exclude_bots,
        hash_length,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

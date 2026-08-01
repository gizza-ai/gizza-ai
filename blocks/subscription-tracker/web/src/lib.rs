//! Browser-facing wasm-bindgen wrapper for /tools/subscription-tracker/.
//! The standalone page passes every field value as a string; the arg order here
//! matches the page/meta.toml `[[input]]` order (subscriptions, default_cycle,
//! currency, sort) and delegates to the shared pure-compute core.
use wasm_bindgen::prelude::*;

/// Total a pasted subscription list. Each arg arrives as a string from the page:
/// - `subscriptions`: one `Name: amount [cycle]` per line.
/// - `default_cycle`: cadence for lines that omit one (blank → monthly).
/// - `currency`: display symbol (blank → `$`).
/// - `sort`: row order — `cost` (default), `name`, or `input`.
///
/// Throws a JS error string on a bad option value or an unparseable list.
#[wasm_bindgen]
pub fn run(
    subscriptions: &str,
    default_cycle: &str,
    currency: &str,
    sort: &str,
) -> Result<String, JsValue> {
    gizza_ai_subscription_tracker_core::track(subscriptions, default_cycle, currency, sort)
        .map_err(|e| JsValue::from_str(&e))
}

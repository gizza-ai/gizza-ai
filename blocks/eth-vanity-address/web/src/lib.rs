//! Browser-facing wasm-bindgen wrapper for /tools/eth-vanity-address/.
//!
//! The page driver hands every field through as a raw string, so each param is
//! taken as `&str` and parsed here; the core owns all validation so the page,
//! the CLI and chat funnel through exactly the same rules. Randomness comes
//! from `crypto.getRandomValues` via getrandom's `js` feature.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false"; treat any positive spelling as on.
fn truthy(v: &str, default: bool) -> bool {
    match v.trim() {
        "" => default,
        s => matches!(s, "true" | "1" | "on" | "yes"),
    }
}

#[wasm_bindgen]
pub fn run(
    prefix: &str,
    suffix: &str,
    match_case: &str,
    max_attempts: &str,
    seed: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    let attempts = match max_attempts.trim() {
        "" => gizza_ai_eth_vanity_address_core::DEFAULT_MAX_ATTEMPTS,
        s => s.parse::<u64>().map_err(|_| {
            JsValue::from_str(&format!(
                "max_attempts must be a whole number between 1 and {}, got '{s}'",
                gizza_ai_eth_vanity_address_core::MAX_ATTEMPTS_CAP
            ))
        })?,
    };
    let start = gizza_ai_eth_vanity_address_core::resolve_start_key(seed)
        .map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_eth_vanity_address_core::run(
        prefix,
        suffix,
        truthy(match_case, false),
        attempts,
        output_format,
        seed,
        &start,
    )
    .map_err(|e| JsValue::from_str(&e))
}

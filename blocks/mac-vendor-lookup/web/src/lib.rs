//! Browser-facing wasm-bindgen wrapper for /tools/mac-vendor-lookup/.
//! Compiled with wasm-pack for the standalone /tools/mac-vendor-lookup/ page.
use wasm_bindgen::prelude::*;

/// Resolve the manufacturer of a MAC / EUI address from its OUI prefix and
/// return the human-readable report. Always succeeds — a malformed input is
/// reported as an `Error:` line, not thrown.
#[wasm_bindgen]
pub fn run(mac: &str) -> Result<String, JsValue> {
    Ok(gizza_ai_mac_vendor_lookup_core::report(mac))
}

//! Browser-facing wasm-bindgen wrapper for /tools/generate-uuid/.
//! Field order MUST match meta.toml: version, count, namespace, name,
//! uppercase, hyphens, braces, urn.
use gizza_ai_generate_uuid_core::generate;
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as the strings "true"/"false"/"on"/etc.
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    version: &str,
    count: f64,
    namespace: &str,
    name: &str,
    uppercase: &str,
    hyphens: &str,
    braces: &str,
    urn: &str,
) -> Result<String, JsValue> {
    let ver = if version.trim().is_empty() { "4" } else { version };
    let cnt = if count >= 1.0 { count.round() as usize } else { 1 };
    let now_ms = (js_sys::Date::now()) as u64;
    let r = generate(
        ver,
        cnt,
        now_ms,
        namespace,
        name,
        truthy(uppercase),
        truthy(hyphens),
        truthy(braces),
        truthy(urn),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(r.uuids.join("\n"))
}

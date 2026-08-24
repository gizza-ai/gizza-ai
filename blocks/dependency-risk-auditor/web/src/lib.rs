//! Browser-facing wasm-bindgen wrapper for /tools/dependency-risk-auditor/.
//! Field order MUST match page/meta.toml.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as `"true"`/`"false"`; an empty value means the field
/// was absent, so fall back to the descriptor default.
fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        default
    } else {
        matches!(t.as_str(), "true" | "1" | "on" | "yes")
    }
}

/// Empty select/text values fall back to the descriptor default so a deep link
/// that omits a param behaves exactly like the chat/CLI surfaces.
fn or_default<'a>(s: &'a str, default: &'a str) -> &'a str {
    if s.trim().is_empty() {
        default
    } else {
        s
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    manifest: &str,
    lockfile: &str,
    manifest_format: &str,
    strictness: &str,
    include_dev: &str,
    ignore: &str,
    fail_on: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_dependency_risk_auditor_core::audit(
        manifest,
        lockfile,
        or_default(manifest_format, "auto"),
        or_default(strictness, "standard"),
        truthy(include_dev, true),
        ignore,
        or_default(fail_on, "high"),
        or_default(output, "text"),
    )
    .map_err(|e| JsValue::from_str(&e))
}

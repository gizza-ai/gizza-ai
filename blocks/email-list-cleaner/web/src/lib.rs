//! Browser-facing wasm-bindgen wrapper for /tools/email-list-cleaner/.
//! Field order MUST match page/meta.toml: emails, canonicalize, sort, format.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(emails: &str, canonicalize: &str, sort: &str, format: &str) -> Result<String, JsValue> {
    let canonicalize = matches!(
        canonicalize.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let sort_alpha = match sort.trim().to_ascii_lowercase().as_str() {
        "" | "input" => false,
        "alpha" => true,
        other => {
            return Err(JsValue::from_str(&format!(
                "invalid sort {other:?}: expected 'input' or 'alpha'"
            )))
        }
    };
    let format = if format.trim().is_empty() { "report" } else { format };
    gizza_ai_email_list_cleaner_core::report(emails, canonicalize, sort_alpha, format)
        .map_err(|e| JsValue::from_str(&e))
}

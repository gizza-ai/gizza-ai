//! Browser-facing wasm-bindgen wrapper for /tools/url-stripper/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the bool fields here;
//! the core owns the stripping logic. Param order MUST match page/meta.toml's
//! [[input]] order: input, remove_emails, remove_www, replacement,
//! collapse_whitespace.
use gizza_ai_url_stripper_core::{render, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    remove_emails: &str,
    remove_www: &str,
    replacement: &str,
    collapse_whitespace: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        remove_emails: truthy(remove_emails, false),
        remove_www: truthy(remove_www, true),
        replacement: replacement.to_string(),
        collapse_whitespace: truthy(collapse_whitespace, true),
    };
    render(input, &opts).map_err(|e| JsValue::from_str(&e))
}

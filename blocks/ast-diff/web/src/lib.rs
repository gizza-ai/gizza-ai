//! Browser-facing wasm-bindgen wrapper for /tools/ast-diff/.
//! Compiled with wasm-pack for the standalone /tools/ast-diff/ page.
use wasm_bindgen::prelude::*;

/// Structurally diff two Rust sources `a` and `b`.
///
/// The tool page passes every field value as a string, so `context` arrives as
/// a string and is parsed here (blank / unparseable → the default of 3). `mode`
/// is one of `unified` (blank → this), `summary`, `canonical-a`, `canonical-b`.
///
/// Throws a JS error string when either source isn't valid Rust or `mode` is
/// unknown.
#[wasm_bindgen]
pub fn run(a: &str, b: &str, mode: &str, context: &str) -> Result<String, JsValue> {
    let context = context.trim().parse::<usize>().unwrap_or(3);
    gizza_ai_ast_diff_core::diff(a, b, mode, context).map_err(|e| JsValue::from_str(&e))
}

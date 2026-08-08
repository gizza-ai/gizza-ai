//! Browser-facing wasm-bindgen wrapper for /tools/phylip-to-fasta/.
//! Compiled with wasm-pack for the standalone page; every field arrives as a
//! string, so the numeric/boolean/enum params are parsed here.
use gizza_ai_phylip_to_fasta_core::{convert, Layout, NameStyle, Options};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Convert PHYLIP alignment text to FASTA.
///
/// - `phylip`: the PHYLIP text, starting with the `<taxa> <sites>` header.
/// - `layout`: `"auto"` (default/blank), `"sequential"` or `"interleaved"`.
/// - `name_style`: `"auto"` (default/blank), `"strict"` or `"relaxed"`.
/// - `wrap`: FASTA line width (blank/unparseable → 60; `0` = one line; core caps at 1000).
/// - `uppercase` / `remove_gaps` / `tolerant`: `"true"`/`"1"`/`"yes"`/`"on"` → on.
///
/// Throws a JS error string on malformed PHYLIP or an invalid enum value.
#[wasm_bindgen]
pub fn run(
    phylip: &str,
    layout: &str,
    name_style: &str,
    wrap: &str,
    uppercase: &str,
    remove_gaps: &str,
    tolerant: &str,
) -> Result<String, JsValue> {
    let layout = match layout.trim() {
        "" | "auto" => Layout::Auto,
        "sequential" => Layout::Sequential,
        "interleaved" => Layout::Interleaved,
        other => {
            return Err(JsValue::from_str(&format!(
                "invalid layout {other:?}: expected \"auto\", \"sequential\" or \"interleaved\""
            )))
        }
    };
    let name_style = match name_style.trim() {
        "" | "auto" => NameStyle::Auto,
        "strict" => NameStyle::Strict,
        "relaxed" => NameStyle::Relaxed,
        other => {
            return Err(JsValue::from_str(&format!(
                "invalid name_style {other:?}: expected \"auto\", \"strict\" or \"relaxed\""
            )))
        }
    };
    let opts = Options {
        layout,
        name_style,
        // A blank field means "leave it at the documented default", not 0.
        wrap: match wrap.trim() {
            "" => 60,
            w => w.parse::<usize>().unwrap_or(60),
        },
        uppercase: truthy(uppercase),
        remove_gaps: truthy(remove_gaps),
        tolerant: truthy(tolerant),
    };
    convert(phylip, &opts).map_err(|e| JsValue::from_str(&e))
}

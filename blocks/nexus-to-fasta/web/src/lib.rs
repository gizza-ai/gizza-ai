//! Browser-facing wasm-bindgen wrapper for /tools/nexus-to-fasta/.
//! Compiled with wasm-pack for the standalone page; every field arrives as a
//! string, so the numeric/boolean/enum params are parsed here.
use gizza_ai_nexus_to_fasta_core::{convert, Case, Layout, Options};
use wasm_bindgen::prelude::*;

/// Checkbox fields arrive as "true"/"false" — match positively so an unexpected
/// value reads as "off" rather than silently flipping the flag on.
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Convert NEXUS alignment text to FASTA.
///
/// - `nexus`: the NEXUS document, including its `begin data;` … `end;` block.
/// - `layout`: `"auto"` (default/blank), `"sequential"` or `"interleaved"`.
/// - `wrap`: FASTA line width (blank/unparseable → 60; `0` = one line; core caps at 1000).
/// - `case`: `"keep"` (default/blank), `"upper"` or `"lower"`.
/// - `remove_gaps` / `expand_matchchar` / `underscores_to_spaces` / `tolerant`:
///   `"true"`/`"1"`/`"yes"`/`"on"` → on.
///
/// Throws a JS error string on malformed NEXUS or an invalid enum value.
#[wasm_bindgen]
pub fn run(
    nexus: &str,
    layout: &str,
    wrap: &str,
    case: &str,
    remove_gaps: &str,
    expand_matchchar: &str,
    underscores_to_spaces: &str,
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
    let case = match case.trim() {
        "" | "keep" => Case::Keep,
        "upper" => Case::Upper,
        "lower" => Case::Lower,
        other => {
            return Err(JsValue::from_str(&format!(
                "invalid case {other:?}: expected \"keep\", \"upper\" or \"lower\""
            )))
        }
    };
    let opts = Options {
        layout,
        // A blank field means "leave it at the documented default", not 0.
        wrap: match wrap.trim() {
            "" => 60,
            w => w.parse::<usize>().unwrap_or(60),
        },
        case,
        remove_gaps: truthy(remove_gaps),
        expand_matchchar: truthy(expand_matchchar),
        underscores_to_spaces: truthy(underscores_to_spaces),
        tolerant: truthy(tolerant),
    };
    convert(nexus, &opts).map_err(|e| JsValue::from_str(&e))
}

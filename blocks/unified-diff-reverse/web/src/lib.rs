//! Browser-facing wasm-bindgen wrapper for /tools/unified-diff-reverse/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Invert a pasted unified diff into the patch that undoes it.
///
/// The standalone tool page passes every field value as a string (and in the
/// `page/meta.toml` `[[input]]` order, which mirrors the descriptor's param
/// order), so the boolean arrives as a string and is parsed here:
/// - `diff`:        the pasted unified / `git diff` patch (1 MB cap).
/// - `output`:      `patch` (blank) | `summary` | `json`.
/// - `file`:        one path out of a multi-file patch. Blank reverses every file.
/// - `index_lines`: `swap` (blank) | `keep` | `drop` for git `index` lines.
/// - `swap_paths`:  default-on checkbox — only an explicit falsey string turns it off.
/// - `on_binary`:   `fail` (blank) | `skip` | `keep` for binary file sections.
///
/// Throws a JS error string on an empty/unparseable patch, an unknown option
/// value, a `file` selector that matches nothing, an un-invertible binary
/// section under `on_binary = "fail"`, or input over the cap.
#[wasm_bindgen]
pub fn run(
    diff: &str,
    output: &str,
    file: &str,
    index_lines: &str,
    swap_paths: &str,
    on_binary: &str,
) -> Result<String, JsValue> {
    // Default-on checkbox: only an explicit falsey string turns the swap off.
    let swap_paths = !matches!(
        swap_paths.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    );
    gizza_ai_unified_diff_reverse_core::reverse_diff(
        diff,
        output,
        file,
        index_lines,
        swap_paths,
        on_binary,
    )
    .map_err(|e| JsValue::from_str(&e))
}

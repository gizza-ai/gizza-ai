//! Browser-facing wasm-bindgen wrapper for /tools/diff-hunk-selector/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// List / filter / split the hunks of a pasted unified diff.
///
/// The standalone tool page passes every field value as a string (and in the
/// `page/meta.toml` `[[input]]` order, which mirrors the descriptor's param
/// order), so the booleans arrive as strings and are parsed here:
/// - `diff`:     the pasted unified / `git diff` patch (1 MB cap).
/// - `output`:   `list` (blank) | `patch` | `split` | `json`.
/// - `hunks`:    `all` (blank) | `2` | `1,3-5` | `4-` | `-2`.
/// - `invert`:   default-off checkbox — `"true"`/`"1"`/`"yes"`/`"on"` turns it on.
/// - `files`:    comma-separated globs; a `!` prefix excludes. Blank keeps every file.
/// - `lines`:    span grammar matched against original-file line numbers. Blank keeps all.
/// - `renumber`: default-on checkbox — only an explicit falsey string turns it off.
///
/// Throws a JS error string on an empty/unparseable diff, an invalid selection,
/// an empty selection for `patch`/`split`, or input over the cap.
#[wasm_bindgen]
pub fn run(
    diff: &str,
    output: &str,
    hunks: &str,
    invert: &str,
    files: &str,
    lines: &str,
    renumber: &str,
) -> Result<String, JsValue> {
    // Default-off checkbox: only an explicit truthy string turns inversion on.
    let invert = matches!(
        invert.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    // Default-on checkbox: only an explicit falsey string turns renumbering off.
    let renumber = !matches!(
        renumber.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    );
    gizza_ai_diff_hunk_selector_core::select_hunks(
        diff, output, hunks, invert, files, lines, renumber,
    )
    .map_err(|e| JsValue::from_str(&e))
}

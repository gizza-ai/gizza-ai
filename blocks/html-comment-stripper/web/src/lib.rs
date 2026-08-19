//! Browser-facing wasm-bindgen wrapper for /tools/html-comment-stripper/.
//! Argument order MUST match page/meta.toml: html, keep_conditional, keep_ssi,
//! keep_bang, pattern, pattern_mode, remove_css_comments, blank_lines, output.
//! Every field arrives as a string (checkboxes send "true"/"false"); the core
//! owns all validation and error messages.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"on"`/`"yes"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Strip `<!-- … -->` comments from markup, leaving every other byte in place.
///
/// - `html`: the markup to clean (max 5,000,000 bytes).
/// - `keep_conditional`: checkbox `"true"`/`"false"` (default-checked) — protect
///   `<!--[if …]> … <![endif]-->`.
/// - `keep_ssi`: checkbox `"true"`/`"false"` (default-checked) — protect
///   `<!--# … -->` server-side includes.
/// - `keep_bang`: checkbox `"true"`/`"false"` (default-checked) — protect
///   `<!--! … -->` banner/licence comments.
/// - `pattern`: a regular expression over each comment's inner text; blank
///   disables it.
/// - `pattern_mode`: `keep` (matches are protected) | `only` (matches are the
///   only comments removed).
/// - `remove_css_comments`: checkbox `"true"`/`"false"` (default-unchecked) —
///   also strip CSS block comments inside `<style>`.
/// - `blank_lines`: `keep` | `trim` | `collapse`.
/// - `output`: `html` | `report` | `comments`.
///
/// Throws a JS error string on empty input, an invalid pattern, `only` mode
/// without a pattern, an unterminated comment, an unknown option value, or an
/// over-cap document.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    html: &str,
    keep_conditional: &str,
    keep_ssi: &str,
    keep_bang: &str,
    pattern: &str,
    pattern_mode: &str,
    remove_css_comments: &str,
    blank_lines: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_html_comment_stripper_core::strip(
        html,
        truthy(keep_conditional),
        truthy(keep_ssi),
        truthy(keep_bang),
        pattern,
        pattern_mode,
        truthy(remove_css_comments),
        blank_lines,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

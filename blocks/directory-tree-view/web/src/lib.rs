//! Browser-facing wasm-bindgen wrapper for /tools/directory-tree-view/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Render a pasted file listing as a size-annotated directory tree.
///
/// The standalone tool page passes every field value as a string, in the same
/// order as the `[[input]]` blocks in `page/meta.toml`:
/// - `input`: the file listing (path + byte size per line).
/// - `format`: `"auto"`/`"size-first"`/`"path-first"` (blank → auto).
/// - `units`: `"human"`/`"si"`/`"bytes"` (blank → human).
/// - `sort`: `"name"`/`"size-desc"`/`"input"` (blank → name).
/// - `root`: the root label (blank → ".").
/// - `ascii`: truthy → plain-ASCII connectors (default false / off).
/// - `dirs_first`: truthy → directories before files (default true / on).
/// - `trailing_slash`: truthy → add "/" to directories (default true / on).
/// - `show_counts`: truthy → per-directory counts (default true / on).
/// - `depth`: max depth as a number string (blank → 0 = unlimited).
///
/// Throws a JS error string on an invalid enum, bad line, or empty input.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    format: &str,
    units: &str,
    sort: &str,
    root: &str,
    ascii: &str,
    dirs_first: &str,
    trailing_slash: &str,
    show_counts: &str,
    depth: &str,
) -> Result<String, JsValue> {
    let ascii = truthy(ascii, false);
    // Default-true checkboxes: blank means the box is still default-checked.
    let dirs_first = truthy(dirs_first, true);
    let trailing_slash = truthy(trailing_slash, true);
    let show_counts = truthy(show_counts, true);
    let depth = parse_depth(depth).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_directory_tree_view_core::build(
        input,
        format,
        units,
        sort,
        root,
        ascii,
        dirs_first,
        trailing_slash,
        show_counts,
        depth,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Interpret a checkbox value string. Blank → `default` (the box is rendered
/// checked iff the descriptor default is true, and a checked box sends "true").
fn truthy(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

/// Parse the depth number field. Blank → 0 (unlimited).
fn parse_depth(v: &str) -> Result<i64, String> {
    let t = v.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>()
        .map_err(|_| format!("invalid depth {v:?}: expected a whole number (0 = unlimited)"))
}

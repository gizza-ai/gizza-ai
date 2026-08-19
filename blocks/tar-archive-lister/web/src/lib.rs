//! Browser-facing wasm-bindgen wrapper for /tools/tar-archive-lister/.
//! Compiled with wasm-pack for the standalone /tools/tar-archive-lister/ page.
//!
//! Field order MUST match meta.toml: input, input_format, output, sort, filter,
//! include_dirs, time_format, limit. The page passes every field value as a
//! string (checkboxes arrive as "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// List the members of a tar (or tar.gz) archive without unpacking it.
///
/// - `input`: the archive bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `output`: `"table"` (default), `"paths"`, `"csv"` or `"json"`.
/// - `sort`: `"archive"` (default), `"path"`, `"size"`, `"mtime"` or `"type"`.
/// - `filter`: blank, a `*`/`?` glob, or a plain substring.
/// - `include_dirs`: `"true"`/`"false"` — list directory members.
/// - `time_format`: `"iso"` (default), `"epoch"` or `"none"`.
/// - `limit`: maximum members to return (blank → 500).
///
/// Throws a JS error string on invalid arguments or an unparseable archive.
#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    sort: &str,
    filter: &str,
    include_dirs: &str,
    time_format: &str,
    limit: &str,
) -> Result<String, JsValue> {
    let n: usize = match limit.trim() {
        "" => 500,
        other => other
            .parse()
            .map_err(|_| JsValue::from_str(&format!("invalid limit {other:?}: expected a whole number 1-200000")))?,
    };
    gizza_ai_tar_archive_lister_core::run(
        input,
        input_format,
        output,
        sort,
        filter,
        truthy(include_dirs),
        time_format,
        n,
    )
    .map_err(|e| JsValue::from_str(&e))
}

//! Browser-facing wasm-bindgen wrapper for /tools/xml-namespace-stripper/.
//! Argument order MUST match page/meta.toml: xml, mode, keep, conflicts,
//! remove_schema_hints, format, indent, output. Every field arrives as a string
//! (checkboxes send "true"/"false"); the core owns all validation and error
//! messages so the page, the CLI and chat behave identically.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"on"`/`"yes"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. The page's checkbox sends `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Strip XML namespace declarations and element/attribute prefixes.
///
/// - `xml`: the document to strip (max 5,000,000 bytes).
/// - `mode`: `all` | `prefixes` | `declarations`.
/// - `keep`: comma-separated prefixes to leave alone (`xmlns` = the default
///   declaration); blank keeps nothing.
/// - `conflicts`: `rename` | `first` | `error`, for two attributes that collapse
///   onto one name.
/// - `remove_schema_hints`: checkbox `"true"`/`"false"` (default-checked).
/// - `format`: `preserve` | `pretty` | `minify`.
/// - `indent`: spaces per level for `pretty`; blank or unparseable → 2.
/// - `output`: `xml` | `report`.
///
/// Throws a JS error string on empty, oversized or malformed XML, an unknown
/// option value, or an attribute name clash under `conflicts=error`.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    xml: &str,
    mode: &str,
    keep: &str,
    conflicts: &str,
    remove_schema_hints: &str,
    format: &str,
    indent: &str,
    output: &str,
) -> Result<String, JsValue> {
    let indent: usize = indent.trim().parse().unwrap_or(2);
    gizza_ai_xml_namespace_stripper_core::strip(
        xml,
        mode,
        keep,
        conflicts,
        truthy(remove_schema_hints),
        format,
        indent,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

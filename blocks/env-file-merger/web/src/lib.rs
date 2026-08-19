//! Browser-facing wasm-bindgen wrapper for /tools/env-file-merger/.
//! The standalone page passes every field value as a string, in the order the
//! `[[input]]` blocks are declared in page/meta.toml; booleans arrive as
//! "true"/"false" from the checkboxes and are parsed here.
use wasm_bindgen::prelude::*;

/// Merge up to four layered `.env` documents, lowest priority first.
///
/// - `layer1`..`layer4`: the layer texts; blank layers are skipped.
/// - `layer_names`: comma-separated display names for the four slots.
/// - `output`: `report` | `env` | `json` | `shell` | `markdown` | `conflicts`
///   (blank → `report`).
/// - `mask_secrets`: `"true"`/`"1"`/`"yes"`/`"on"` masks sensitive-looking
///   values; the checkbox defaults to checked, so blank means OFF only when the
///   field was explicitly cleared.
/// - `sort_keys`: same truthy forms — sort keys alphabetically.
/// - `prefix_filter`: keep only keys starting with this prefix.
/// - `expand_vars`: same truthy forms — resolve `${VAR}` references.
///
/// Throws a JS error string when every layer is blank or `output` is unknown.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    layer1: &str,
    layer2: &str,
    layer3: &str,
    layer4: &str,
    layer_names: &str,
    output: &str,
    mask_secrets: &str,
    sort_keys: &str,
    prefix_filter: &str,
    expand_vars: &str,
) -> Result<String, JsValue> {
    let output = if output.trim().is_empty() {
        "report"
    } else {
        output.trim()
    };
    gizza_ai_env_file_merger_core::merge(
        &[layer1, layer2, layer3, layer4],
        layer_names,
        output,
        truthy(mask_secrets),
        truthy(sort_keys),
        prefix_filter,
        truthy(expand_vars),
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Positive-truthy checkbox parsing: the page sends "true"/"false", deep links
/// may send "1"/"yes"/"on".
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

//! Browser-facing wasm-bindgen wrapper for /tools/graphql-mock-from-sdl/.
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false"; treat anything not explicitly
/// negative as on (the descriptor default for `smart_values`/`pretty` is true).
fn truthy_default_on(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

/// `typename` defaults to OFF, so only an explicitly positive value enables it.
fn truthy_default_off(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_u32(v: &str, fallback: u32, field: &str) -> Result<u32, JsValue> {
    if v.trim().is_empty() {
        return Ok(fallback);
    }
    v.trim()
        .parse::<u32>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    sdl: &str,
    mode: &str,
    type_name: &str,
    list_length: &str,
    depth: &str,
    nullable_fields: &str,
    smart_values: &str,
    typename: &str,
    seed: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let mode = if mode.trim().is_empty() {
        "all-types"
    } else {
        mode.trim()
    };
    let nullable_fields = if nullable_fields.trim().is_empty() {
        "fill"
    } else {
        nullable_fields.trim()
    };
    let list_length = parse_u32(list_length, 2, "list_length")?;
    let depth = parse_u32(depth, 3, "depth")?;
    let seed = if seed.trim().is_empty() {
        1
    } else {
        seed.trim()
            .parse::<u64>()
            .map_err(|_| JsValue::from_str("seed must be a whole number"))?
    };

    gizza_ai_graphql_mock_from_sdl_core::generate(
        sdl,
        mode,
        type_name,
        list_length,
        depth,
        nullable_fields,
        truthy_default_on(smart_values),
        truthy_default_off(typename),
        seed,
        truthy_default_on(pretty),
    )
    .map_err(|e| JsValue::from_str(&e))
}

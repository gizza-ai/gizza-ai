//! Browser-facing wasm-bindgen wrapper for /tools/shopping-list-builder/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    ingredients: &str,
    scale: f64,
    group_by: &str,
    unit_system: &str,
    exclude: &str,
    checkboxes: &str,
    show_sources: &str,
    format: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| matches!(v, "true" | "1" | "on" | "yes");
    gizza_ai_shopping_list_builder_core::run(
        ingredients,
        scale,
        group_by,
        unit_system,
        exclude,
        truthy(checkboxes),
        truthy(show_sources),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}

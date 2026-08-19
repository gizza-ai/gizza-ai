//! Browser-facing wasm-bindgen wrapper for /tools/compose-to-diagram/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    compose: &str,
    direction: &str,
    networks: &str,
    ports: &str,
    volumes: &str,
    labels: &str,
    profile: &str,
    styled: &str,
    title: &str,
    output: &str,
) -> Result<String, JsValue> {
    fn truthy(s: &str, default: bool) -> bool {
        match s.trim().to_ascii_lowercase().as_str() {
            "" => default,
            "true" | "1" | "on" | "yes" => true,
            "false" | "0" | "off" | "no" => false,
            _ => default,
        }
    }

    gizza_ai_compose_to_diagram_core::compose_to_diagram(
        compose,
        direction,
        networks,
        truthy(ports, true),
        truthy(volumes, true),
        labels,
        profile,
        truthy(styled, true),
        title,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

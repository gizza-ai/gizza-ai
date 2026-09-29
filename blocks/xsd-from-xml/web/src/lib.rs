//! Browser-facing wasm-bindgen wrapper for /tools/xsd-from-xml/.
use wasm_bindgen::prelude::*;

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() { fallback.to_string() } else { s.trim().to_string() }
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<f64>().map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{t}`")))
    }
}

fn parse_bool(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => fallback,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    xml: &str,
    design: &str,
    type_inference: &str,
    occurrence: &str,
    enumerations: &str,
    target_namespace: &str,
    indent: &str,
    declaration: &str,
) -> Result<String, JsValue> {
    gizza_ai_xsd_from_xml_core::run(
        xml,
        &or_default(design, "venetian-blind"),
        &or_default(type_inference, "smart"),
        &or_default(occurrence, "restricted"),
        parse_f64("enumerations", enumerations, 0.0)?,
        target_namespace,
        parse_f64("indent", indent, 2.0)?,
        parse_bool(declaration, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}

//! Browser-facing wasm-bindgen wrapper for /tools/elasticsearch-mapping-generator/.
//! Field ORDER must match page/meta.toml: json, output, text_fields,
//! ignore_above, analyzer, integer_type, float_type, date_detection,
//! numeric_detection, detect_ip, detect_geo_point, array_objects, dynamic,
//! shards, replicas. The page hands every field over as a string, so the
//! parsing lives in the core.
use gizza_ai_elasticsearch_mapping_generator_core::{generate, options_from_strings};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    json: &str,
    output: &str,
    text_fields: &str,
    ignore_above: &str,
    analyzer: &str,
    integer_type: &str,
    float_type: &str,
    date_detection: &str,
    numeric_detection: &str,
    detect_ip: &str,
    detect_geo_point: &str,
    array_objects: &str,
    dynamic: &str,
    shards: &str,
    replicas: &str,
) -> Result<String, JsValue> {
    let opts = options_from_strings(
        output,
        text_fields,
        ignore_above,
        analyzer,
        integer_type,
        float_type,
        date_detection,
        numeric_detection,
        detect_ip,
        detect_geo_point,
        array_objects,
        dynamic,
        shards,
        replicas,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    generate(json, &opts).map_err(|e| JsValue::from_str(&e))
}

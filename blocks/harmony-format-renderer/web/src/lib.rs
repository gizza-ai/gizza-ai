//! Browser-facing wasm-bindgen wrapper for /tools/harmony-format-renderer/.
//! The page driver passes EVERY field as a string, so booleans arrive as
//! "true"/"false" and are parsed here; all validation lives in the core.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    messages: &str,
    input_format: &str,
    instructions: &str,
    tools: &str,
    model_identity: &str,
    reasoning_effort: &str,
    knowledge_cutoff: &str,
    current_date: &str,
    include_system: &str,
    render_target: &str,
    auto_drop_analysis: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    gizza_ai_harmony_format_renderer_core::run(
        messages,
        input_format,
        instructions,
        tools,
        model_identity,
        reasoning_effort,
        knowledge_cutoff,
        current_date,
        truthy(include_system),
        render_target,
        truthy(auto_drop_analysis),
        output_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

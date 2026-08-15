//! Browser-facing wasm-bindgen wrapper for /tools/docker-cli-output-parser/.
//! Compiled with wasm-pack for the standalone page. Field order MUST match
//! page/meta.toml: input, kind, output, keys, parse_values, columns, header,
//! strict, limit.
use wasm_bindgen::prelude::*;

/// Checkboxes arrive as "true"/"false" strings; anything empty keeps the default.
fn parse_bool(value: &str, default: bool) -> bool {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        default
    } else {
        matches!(v.as_str(), "true" | "1" | "yes" | "on")
    }
}

fn parse_limit(value: &str) -> Result<u32, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(gizza_ai_docker_cli_output_parser_core::DEFAULT_LIMIT);
    }
    let n = v
        .parse::<u32>()
        .map_err(|_| JsValue::from_str("limit must be a whole number from 1 to 5000"))?;
    if n < 1 || n > gizza_ai_docker_cli_output_parser_core::MAX_LIMIT {
        return Err(JsValue::from_str(
            "limit must be a whole number from 1 to 5000",
        ));
    }
    Ok(n)
}

/// Parse docker ps/images/stats output into JSON, CSV, TSV, Markdown or a table.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    kind: &str,
    output: &str,
    keys: &str,
    parse_values: &str,
    columns: &str,
    header: &str,
    strict: &str,
    limit: &str,
) -> Result<String, JsValue> {
    gizza_ai_docker_cli_output_parser_core::parse(
        input,
        kind,
        output,
        keys,
        parse_bool(parse_values, true),
        columns,
        parse_bool(header, true),
        parse_bool(strict, false),
        parse_limit(limit)?,
    )
    .map_err(|e| JsValue::from_str(&e))
}

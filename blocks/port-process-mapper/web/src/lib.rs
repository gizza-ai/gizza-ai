//! Browser-facing wasm-bindgen wrapper for /tools/port-process-mapper/.
//! The standalone page passes every field value as a string, so the enum and
//! boolean params arrive as strings and are parsed here.
use gizza_ai_port_process_mapper_core as core;
use wasm_bindgen::prelude::*;

/// A page checkbox sends `"true"`/`"false"`; anything else falls back to the
/// descriptor default so a missing query param keeps the documented behaviour.
fn flag(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => true,
        "false" | "0" | "off" | "no" => false,
        _ => default,
    }
}

/// Map a pasted `lsof`/`ss`/`netstat` listing to a port → PID → process table.
///
/// - `input_format`: `"auto"` (default) | `"lsof"` | `"ss"` | `"netstat"` | `"netstat-windows"`.
/// - `output_format`: `"markdown"` (default) | `"csv"` | `"json"` | `"text"`.
/// - `sort_by`: `"port"` (default) | `"pid"` | `"process"` | `"state"` | `"address"`.
/// - `protocol`: `"any"` (default) | `"tcp"` | `"udp"`.
/// - `ports`: comma-separated numbers/ranges (`80,443,8000-8100`); empty = all.
/// - `process`: case-insensitive command-name substring; empty = all.
#[wasm_bindgen]
pub fn run(
    input: &str,
    input_format: &str,
    output_format: &str,
    sort_by: &str,
    listening_only: &str,
    protocol: &str,
    ports: &str,
    process: &str,
    conflicts_only: &str,
    annotate_services: &str,
    kill_commands: &str,
) -> Result<String, JsValue> {
    core::parse(
        input,
        core::Options {
            input_format: core::InputFormat::parse(input_format),
            output_format: core::OutputFormat::parse(output_format),
            sort_by: core::SortBy::parse(sort_by),
            listening_only: flag(listening_only, true),
            protocol: core::Protocol::parse(protocol),
            ports: ports.to_string(),
            process: process.to_string(),
            conflicts_only: flag(conflicts_only, false),
            annotate_services: flag(annotate_services, true),
            kill_commands: flag(kill_commands, false),
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}

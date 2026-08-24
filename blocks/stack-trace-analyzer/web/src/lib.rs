//! Browser-facing wasm-bindgen wrapper for /tools/stack-trace-analyzer/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Analyse a raw `trace` into its exception chain, frames, and root cause.
///
/// The tool page passes every field value as a string, so the boolean/integer
/// params arrive as strings and are parsed here:
/// - `language`:       blank/`auto` | `java` | `python` | `javascript` | `go` |
///   `ruby` | `csharp` | `rust` | `php`.
/// - `output`:         blank/`report` | `table` | `json`.
/// - `user_packages`:  comma-separated prefixes marking your own code.
/// - `hide_framework`: `"true"`/`"1"`/`"yes"`/`"on"` → drop framework frames.
/// - `reverse`:        same truthy forms → list frames outermost first.
/// - `limit`:          frames per exception, 1–2000 (blank → 0 → the core
///   default of 100).
///
/// Throws a JS error string on empty/oversized input, an unknown
/// `language`/`output`, or a trace whose language can't be detected.
#[wasm_bindgen]
pub fn run(
    trace: &str,
    language: &str,
    output: &str,
    user_packages: &str,
    hide_framework: &str,
    reverse: &str,
    limit: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    };
    let limit = limit.trim().parse::<u32>().unwrap_or(0);
    gizza_ai_stack_trace_analyzer_core::analyze(
        trace,
        language,
        output,
        user_packages,
        truthy(hide_framework),
        truthy(reverse),
        limit,
    )
    .map_err(|e| JsValue::from_str(&e))
}

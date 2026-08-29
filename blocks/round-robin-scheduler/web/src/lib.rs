//! Browser-facing wasm-bindgen wrapper for /tools/round-robin-scheduler/.
//! Field order MUST match meta.toml: participants, schedule_type, output_format, courts,
//! start_round, include_byes, include_summary, seed.
use gizza_ai_round_robin_scheduler_core::{generate, Options, OutputFormat, ScheduleType};
use wasm_bindgen::prelude::*;

/// The page hands every field over as a string, so parse the numeric/boolean ones here.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn number(v: &str, field: &str, default: i64) -> Result<i64, String> {
    let v = v.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<i64>()
        .map_err(|_| format!("{field} must be a whole number, got '{v}'"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    participants: &str,
    schedule_type: &str,
    output_format: &str,
    courts: &str,
    start_round: &str,
    include_byes: &str,
    include_summary: &str,
    seed: &str,
) -> Result<String, JsValue> {
    let err = |e: String| JsValue::from_str(&e);
    let opts = Options {
        schedule_type: ScheduleType::parse(schedule_type).map_err(err)?,
        format: OutputFormat::parse(output_format).map_err(err)?,
        courts: courts.to_string(),
        start_round: number(start_round, "start_round", 1).map_err(err)?,
        include_byes: truthy(include_byes),
        include_summary: truthy(include_summary),
        seed: number(seed, "seed", 0).map_err(err)?.unsigned_abs(),
    };
    generate(participants, &opts).map_err(err)
}

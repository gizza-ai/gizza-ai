//! gizza-ai/human-readable-units — formats raw magnitudes as readable units.
//!
//! Thin chat-skill wrapper around `gizza-ai-human-readable-units-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_human_readable_units_core as core;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    value: String,
    /// "bytes" (default) | "duration" | "number".
    #[serde(default)]
    kind: String,
    /// "auto" (default) or an explicit data/time unit.
    #[serde(default)]
    input_unit: String,
    /// "decimal" (default) | "binary-iec" | "binary-jedec".
    #[serde(default)]
    base: String,
    /// "short" (default) | "long" | "narrow".
    #[serde(default)]
    style: String,
    /// Decimal places on the scaled value (default 1).
    #[serde(default = "default_precision")]
    precision: u32,
    /// Maximum compound duration segments (default 2).
    #[serde(default = "default_max_units")]
    max_units: u32,
    /// Drop trailing zeros from the scaled value (default true).
    #[serde(default = "default_true")]
    trim_zeros: bool,
    /// "value" (default) | "breakdown".
    #[serde(default)]
    detail: String,
}

fn default_precision() -> u32 {
    1
}
fn default_max_units() -> u32 {
    2
}
fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("value")
                .required()
                .describe("The raw magnitude to format, as a number. Digit-group separators are ignored, so 1500000000, 1,500,000,000 and 1_500_000_000 are the same input; scientific notation (1.5e9) and negative values are also accepted. What it counts is set by 'kind' and 'input_unit'. Example: '1500000000' with kind='bytes' returns '1.5 GB'. The converted magnitude must be at most 1e21 base units."),
        )
        .param(
            Param::enumv("kind", ["bytes", "duration", "number"])
                .default("bytes")
                .describe("What the value counts. 'bytes' (default) renders a data size as B/kB/MB/GB…; 'duration' renders a length of time as a compound like '2m 3s'; 'number' renders a plain count with a K/M/B/T suffix like '1.2M'."),
        )
        .param(
            Param::enumv(
                "input_unit",
                [
                    "auto",
                    "byte",
                    "bit",
                    "kilobyte",
                    "megabyte",
                    "gigabyte",
                    "nanosecond",
                    "microsecond",
                    "millisecond",
                    "second",
                    "minute",
                    "hour",
                    "day",
                ],
            )
                .default("auto")
                .describe("The unit the value is already expressed in, converted before formatting. 'auto' (default) means bytes for kind='bytes' and seconds for kind='duration'. Data units (byte, bit, kilobyte=1000 B, megabyte=1e6 B, gigabyte=1e9 B) are only valid with kind='bytes'; time units (nanosecond … day) only with kind='duration'; kind='number' accepts only 'auto'. Example: value='11722000' with kind='duration' and input_unit='millisecond' returns '3h 15m'."),
        )
        .param(
            Param::enumv("base", ["decimal", "binary-iec", "binary-jedec"])
                .default("decimal")
                .describe("Which power and suffix family the byte scale uses (kind='bytes' only, ignored otherwise). 'decimal' (default) divides by 1000 and labels kB/MB/GB — what drive vendors and macOS report. 'binary-iec' divides by 1024 and labels KiB/MiB/GiB. 'binary-jedec' divides by 1024 but keeps the KB/MB/GB labels — what Windows and 'ls -h' show."),
        )
        .param(
            Param::enumv("style", ["short", "long", "narrow"])
                .default("short")
                .describe("How much of the unit name to spell out. 'short' (default) uses symbols with conventional spacing: '1.5 GB', '2m 3s', '1.2M'. 'long' spells them out: '1.5 gigabytes', '2 minutes 3 seconds', '1.2 million'. 'narrow' removes every space: '1.5GB', '2m3s', '1.2M'."),
        )
        .param(
            // Bounds reference the core caps so the LLM-facing schema can't
            // drift from what `format` actually enforces.
            Param::integer("precision")
                .default(1)
                .min(0.0)
                .max(core::MAX_PRECISION as f64)
                .describe("Decimal places on the scaled value, 0-6 (default 1). Applies to kind='bytes' and kind='number'; compound durations are always whole segments. precision=0 gives '2 GB', precision=2 gives '1.47 GiB'."),
        )
        .param(
            Param::integer("max_units")
                .default(2)
                .min(1.0)
                .max(core::MAX_UNITS as f64)
                .describe("Maximum number of segments in a compound duration, 1-7 (default 2); ignored for the other kinds. Segments run d, h, m, s, ms, us, ns, are truncated rather than rounded up, and empty ones are skipped — 3661 seconds is '1h 1m' at 2 units and '1h 1m 1s' at 3."),
        )
        .param(
            Param::boolean("trim_zeros")
                .default(true)
                .describe("When true (default) trailing zeros are dropped from the scaled value, so 1000000000 bytes reads '1 GB'. Set false to keep the fixed number of decimals given by 'precision', which reads '1.0 GB' — useful for aligning a column of sizes."),
        )
        .param(
            Param::enumv("detail", ["value", "breakdown"])
                .default("value")
                .describe("How much to return. 'value' (default) returns just the formatted string. 'breakdown' returns labelled lines with the exact amount and every alternate form: decimal/binary/bits for a size; short, long, HH:MM:SS clock and ISO 8601 for a duration; short, long, grouped and scientific for a count."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Map the string/number args onto the core's typed options.
fn to_options(a: &Args) -> Result<core::Options, String> {
    Ok(core::Options {
        kind: core::parse_kind(&a.kind)?,
        input_unit: core::parse_input_unit(&a.input_unit)?,
        base: core::parse_base(&a.base)?,
        style: core::parse_style(&a.style)?,
        precision: a.precision,
        max_units: a.max_units,
        trim_zeros: a.trim_zeros,
        detail: core::parse_detail(&a.detail)?,
    })
}

#[cfg(target_arch = "wasm32")]
struct HumanReadableUnits;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/human-readable-units",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Format raw byte counts, durations and large numbers as readable units.",
    skill(
        description = "Turn a raw magnitude into the string a person reads: 1500000000 bytes into '1.5 GB', 123 seconds into '2m 3s', 1200000 into '1.2M'. Set kind='bytes' (default), 'duration' or 'number'. input_unit converts first, so a millisecond timing or a megabyte figure can be passed as-is (input_unit='millisecond', 'megabyte', 'bit', …). base picks the byte scale: 'decimal' (1000, kB/MB/GB — the default), 'binary-iec' (1024, KiB/MiB/GiB) or 'binary-jedec' (1024, KB/MB/GB). style is 'short' (1.5 GB), 'long' (1.5 gigabytes) or 'narrow' (1.5GB). precision sets the decimals (0-6, default 1) and trim_zeros=false keeps them fixed for aligned columns. max_units caps compound duration segments (default 2, so 3661 seconds is '1h 1m'). detail='breakdown' also returns the exact amount, the decimal/binary/bits equivalents for a size, or the clock and ISO 8601 forms for a duration. Example: value='123', kind='duration' returns '2m 3s'.",
        parameters = schema_json()
    ),
)]
impl HumanReadableUnits {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "human-readable-units", |a: Args| {
            let opts = to_options(&a).map_err(SkillError::InvalidArgs)?;
            core::format(&a.value, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "value": { "type": "string", "description": "The raw magnitude to format, as a number. Digit-group separators are ignored, so 1500000000, 1,500,000,000 and 1_500_000_000 are the same input; scientific notation (1.5e9) and negative values are also accepted. What it counts is set by 'kind' and 'input_unit'. Example: '1500000000' with kind='bytes' returns '1.5 GB'. The converted magnitude must be at most 1e21 base units." },
                    "kind": { "type": "string", "enum": ["bytes", "duration", "number"], "default": "bytes", "description": "What the value counts. 'bytes' (default) renders a data size as B/kB/MB/GB…; 'duration' renders a length of time as a compound like '2m 3s'; 'number' renders a plain count with a K/M/B/T suffix like '1.2M'." },
                    "input_unit": { "type": "string", "enum": ["auto", "byte", "bit", "kilobyte", "megabyte", "gigabyte", "nanosecond", "microsecond", "millisecond", "second", "minute", "hour", "day"], "default": "auto", "description": "The unit the value is already expressed in, converted before formatting. 'auto' (default) means bytes for kind='bytes' and seconds for kind='duration'. Data units (byte, bit, kilobyte=1000 B, megabyte=1e6 B, gigabyte=1e9 B) are only valid with kind='bytes'; time units (nanosecond … day) only with kind='duration'; kind='number' accepts only 'auto'. Example: value='11722000' with kind='duration' and input_unit='millisecond' returns '3h 15m'." },
                    "base": { "type": "string", "enum": ["decimal", "binary-iec", "binary-jedec"], "default": "decimal", "description": "Which power and suffix family the byte scale uses (kind='bytes' only, ignored otherwise). 'decimal' (default) divides by 1000 and labels kB/MB/GB — what drive vendors and macOS report. 'binary-iec' divides by 1024 and labels KiB/MiB/GiB. 'binary-jedec' divides by 1024 but keeps the KB/MB/GB labels — what Windows and 'ls -h' show." },
                    "style": { "type": "string", "enum": ["short", "long", "narrow"], "default": "short", "description": "How much of the unit name to spell out. 'short' (default) uses symbols with conventional spacing: '1.5 GB', '2m 3s', '1.2M'. 'long' spells them out: '1.5 gigabytes', '2 minutes 3 seconds', '1.2 million'. 'narrow' removes every space: '1.5GB', '2m3s', '1.2M'." },
                    "precision": { "type": "integer", "minimum": 0, "maximum": 6, "default": 1, "description": "Decimal places on the scaled value, 0-6 (default 1). Applies to kind='bytes' and kind='number'; compound durations are always whole segments. precision=0 gives '2 GB', precision=2 gives '1.47 GiB'." },
                    "max_units": { "type": "integer", "minimum": 1, "maximum": 7, "default": 2, "description": "Maximum number of segments in a compound duration, 1-7 (default 2); ignored for the other kinds. Segments run d, h, m, s, ms, us, ns, are truncated rather than rounded up, and empty ones are skipped — 3661 seconds is '1h 1m' at 2 units and '1h 1m 1s' at 3." },
                    "trim_zeros": { "type": "boolean", "default": true, "description": "When true (default) trailing zeros are dropped from the scaled value, so 1000000000 bytes reads '1 GB'. Set false to keep the fixed number of decimals given by 'precision', which reads '1.0 GB' — useful for aligning a column of sizes." },
                    "detail": { "type": "string", "enum": ["value", "breakdown"], "default": "value", "description": "How much to return. 'value' (default) returns just the formatted string. 'breakdown' returns labelled lines with the exact amount and every alternate form: decimal/binary/bits for a size; short, long, HH:MM:SS clock and ISO 8601 for a duration; short, long, grouped and scientific for a count." }
                },
                "required": ["value"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_produce_the_documented_options() {
        let a: Args = serde_json::from_str(r#"{"value":"1500000000"}"#).unwrap();
        let o = to_options(&a).unwrap();
        assert_eq!(o, core::Options::default());
        assert_eq!(core::format(&a.value, &o).unwrap(), "1.5 GB");
    }

    #[test]
    fn an_unknown_enum_value_is_rejected_by_name() {
        let a: Args = serde_json::from_str(r#"{"value":"1000","kind":"bits"}"#).unwrap();
        let err = to_options(&a).unwrap_err();
        assert!(err.contains("invalid kind"), "{err}");
    }

    #[test]
    fn a_duration_in_milliseconds_round_trips_through_args() {
        let a: Args = serde_json::from_str(
            r#"{"value":"11722000","kind":"duration","input_unit":"millisecond","detail":"breakdown"}"#,
        )
        .unwrap();
        let o = to_options(&a).unwrap();
        let out = core::format(&a.value, &o).unwrap();
        assert!(out.starts_with("Formatted: 3h 15m\n"), "{out}");
        assert!(out.contains("ISO 8601: PT3H15M22S"), "{out}");
    }
}

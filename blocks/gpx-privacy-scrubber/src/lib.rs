//! gizza-ai/gpx-privacy-scrubber — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    gpx: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_radius")]
    radius_m: u32,
    #[serde(default = "default_true")]
    scrub_timestamps: bool,
    #[serde(default = "default_true")]
    remove_extensions: bool,
}
fn default_mode() -> String {
    "remove".to_string()
}
fn default_radius() -> u32 {
    200
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("gpx")
                .required()
                .describe("The GPX file to scrub, as XML text. Paste the whole document; every element except the ones being scrubbed is preserved byte-for-byte."),
        )
        .param(
            Param::enumv("mode", ["remove", "fuzz"])
                .default("remove")
                .describe("How to handle the privacy-sensitive first and last track/route/way points. `remove` drops those two point elements entirely; `fuzz` keeps them but nudges their coordinates a deterministic offset within `radius_m` metres. Default remove."),
        )
        .param(
            Param::integer("radius_m")
                .default(200)
                .min(1.0)
                .max(10000.0)
                .describe("Fuzz offset in metres (only used when mode is `fuzz`): the first and last points are moved this far, in opposite directions. 1–10000, default 200."),
        )
        .param(
            Param::boolean("scrub_timestamps")
                .default(true)
                .describe("Replace the contents of every `<time>…</time>` (metadata and every point) with the Unix epoch `1970-01-01T00:00:00Z`, so the recording date and pace are not leaked. Default true."),
        )
        .param(
            Param::boolean("remove_extensions")
                .default(true)
                .describe("Delete every `<extensions>…</extensions>` block — heart rate, cadence, power, temperature, hdop/vdop, and other sensor data. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gpx-privacy-scrubber",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip or fuzz the home/start/end locations, timestamps, and sensor data in a GPX file before sharing it.",
    skill(
        description = "Scrub the privacy-sensitive parts of a GPX file before you share it. The first and last track/route/way points leak your home or start/end location: `mode` = `remove` drops those two point elements, `mode` = `fuzz` keeps them but offsets their coordinates a deterministic distance within `radius_m` metres (1–10000, default 200). `scrub_timestamps` (default on) rewrites every `<time>` (metadata and every point) to the Unix epoch so the recording date and pace are hidden; `remove_extensions` (default on) deletes every `<extensions>` block (heart rate, cadence, power, temperature, hdop/vdop, …). All other content is preserved byte-for-byte. Errors on empty input or a document with no <trkpt>/<rtept>/<wpt> points. Pure and deterministic; runs locally, no network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-privacy-scrubber", |a: Args| {
            gizza_ai_gpx_privacy_scrubber_core::run(
                &a.gpx,
                &a.mode,
                a.radius_m,
                a.scrub_timestamps,
                a.remove_extensions,
            )
            .map_err(SkillError::InvalidArgs)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "gpx": { "type": "string", "description": "The GPX file to scrub, as XML text. Paste the whole document; every element except the ones being scrubbed is preserved byte-for-byte." },
                    "mode": { "type": "string", "enum": ["remove", "fuzz"], "default": "remove", "description": "How to handle the privacy-sensitive first and last track/route/way points. `remove` drops those two point elements entirely; `fuzz` keeps them but nudges their coordinates a deterministic offset within `radius_m` metres. Default remove." },
                    "radius_m": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 200, "description": "Fuzz offset in metres (only used when mode is `fuzz`): the first and last points are moved this far, in opposite directions. 1–10000, default 200." },
                    "scrub_timestamps": { "type": "boolean", "default": true, "description": "Replace the contents of every `<time>…</time>` (metadata and every point) with the Unix epoch `1970-01-01T00:00:00Z`, so the recording date and pace are not leaked. Default true." },
                    "remove_extensions": { "type": "boolean", "default": true, "description": "Delete every `<extensions>…</extensions>` block — heart rate, cadence, power, temperature, hdop/vdop, and other sensor data. Default true." }
                },
                "required": ["gpx"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

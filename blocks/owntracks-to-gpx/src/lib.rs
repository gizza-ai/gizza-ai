//! gizza-ai/owntracks-to-gpx — chat skill block on the shared tool abstraction.
//! Converts an OwnTracks location export (a JSON array / Recorder API object /
//! single location object, or the tab-separated `.rec` recorder format) into a
//! GPX 1.1 track: each `location` fix becomes a `<trkpt lat lon>` with `<ele>`
//! (from `alt`) and `<time>` (from `tst`, formatted as ISO-8601 UTC). The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_owntracks_to_gpx_core::{convert, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    track_name: String,
    #[serde(default = "default_true")]
    include_extensions: bool,
    #[serde(default)]
    segment_gap_minutes: f64,
    #[serde(default)]
    max_accuracy_meters: f64,
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "The OwnTracks location export to convert, pasted as text. Either JSON (an \
                     array of location objects from `ocat --format json`, the Recorder HTTP API's \
                     {\"count\":N,\"data\":[…]} object, or a single location object) or the \
                     Recorder `.rec` format (one record per line, tab-separated: an ISO timestamp, \
                     a type column such as `*`, then the JSON payload). Only _type=\"location\" \
                     records are converted.",
                ),
        )
        .param(
            Param::string("track_name")
                .describe(
                    "Optional name for the output track (emitted as <trk><name>). Leave empty for \
                     no name. Example: \"Sunday hike\".",
                ),
        )
        .param(
            Param::boolean("include_extensions")
                .default(true)
                .describe(
                    "Emit each fix's accuracy, velocity, course, battery, and tracker id as \
                     <extensions> in the OwnTracks namespace (GPX 1.1 has no core element for any \
                     of them). Set false for plain GPX with only latitude/longitude/elevation/time. \
                     Default true.",
                ),
        )
        .param(
            Param::number("segment_gap_minutes")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Start a new <trkseg> whenever the gap between consecutive fixes exceeds this \
                     many minutes (splits a continuous log into per-trip segments). 0 keeps every \
                     point in one segment. Default 0.",
                ),
        )
        .param(
            Param::number("max_accuracy_meters")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Drop fixes whose reported accuracy (acc, in metres) is worse than this — a \
                     larger acc means a less certain position. 0 keeps every point; fixes without \
                     an acc value are always kept. Default 0.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/owntracks-to-gpx",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an OwnTracks location export (JSON or .rec) into a GPX 1.1 track",
    skill(
        description = "Convert an OwnTracks location export into a standard GPX 1.1 track. Accepts either JSON (an array of OwnTracks location objects from `ocat --format json`, the Recorder HTTP API's {\"count\":N,\"data\":[…]} object, or a single location object) or the Recorder `.rec` format (one record per line, tab-separated: an ISO timestamp, a type column such as `*`, then the JSON payload). Only _type=\"location\" records are converted (transitions, waypoints, lwt, etc. are skipped; a record with no _type but a lat/lon pair is still accepted). Each fix becomes a <trkpt lat lon> carrying <ele> (from alt) and <time> (from the tst epoch, formatted as ISO-8601 UTC; a .rec line's own ISO timestamp is used when tst is absent). All points go into one <trk>; set segment_gap_minutes to break it into <trkseg>s wherever the gap between fixes exceeds that many minutes. With include_extensions=true (the default), each point's accuracy/velocity/course/battery/tracker-id are emitted as <extensions> in an OwnTracks namespace. Set max_accuracy_meters to drop imprecise fixes. Paste the export as text; runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "owntracks-to-gpx", |a: Args| {
            let opt = Options {
                track_name: a.track_name,
                include_extensions: a.include_extensions,
                segment_gap_minutes: a.segment_gap_minutes,
                max_accuracy_meters: a.max_accuracy_meters,
            };
            convert(&a.input, &opt).map_err(SkillError::InvalidArgs)
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
            r##"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The OwnTracks location export to convert, pasted as text. Either JSON (an array of location objects from `ocat --format json`, the Recorder HTTP API's {\"count\":N,\"data\":[…]} object, or a single location object) or the Recorder `.rec` format (one record per line, tab-separated: an ISO timestamp, a type column such as `*`, then the JSON payload). Only _type=\"location\" records are converted." },
                    "track_name": { "type": "string", "description": "Optional name for the output track (emitted as <trk><name>). Leave empty for no name. Example: \"Sunday hike\"." },
                    "include_extensions": { "type": "boolean", "default": true, "description": "Emit each fix's accuracy, velocity, course, battery, and tracker id as <extensions> in the OwnTracks namespace (GPX 1.1 has no core element for any of them). Set false for plain GPX with only latitude/longitude/elevation/time. Default true." },
                    "segment_gap_minutes": { "type": "number", "default": 0.0, "minimum": 0, "description": "Start a new <trkseg> whenever the gap between consecutive fixes exceeds this many minutes (splits a continuous log into per-trip segments). 0 keeps every point in one segment. Default 0." },
                    "max_accuracy_meters": { "type": "number", "default": 0.0, "minimum": 0, "description": "Drop fixes whose reported accuracy (acc, in metres) is worse than this — a larger acc means a less certain position. 0 keeps every point; fixes without an acc value are always kept. Default 0." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

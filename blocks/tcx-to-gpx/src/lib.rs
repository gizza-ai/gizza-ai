//! gizza-ai/tcx-to-gpx — chat skill block on the shared tool abstraction.
//! Converts Garmin Training Center XML (TCX) workout files into GPX 1.1,
//! preserving trackpoints (latitude/longitude/elevation/time) plus basic
//! metadata (sport, name) and, optionally, heart rate/cadence as GPX
//! TrackPointExtension elements. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_tcx_to_gpx_core::{convert, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_true")]
    include_extensions: bool,
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
                    "The TCX (Garmin Training Center XML) document to convert, pasted as text \
                     (the contents of a .tcx file — a <TrainingCenterDatabase> with \
                     <Activities>/<Activity> or <Courses>/<Course> holding \
                     <Track>/<Trackpoint> elements).",
                ),
        )
        .param(
            Param::boolean("include_extensions")
                .default(true)
                .describe(
                    "Emit each trackpoint's heart rate and cadence as Garmin TrackPointExtension \
                     elements (<gpxtpx:hr>/<gpxtpx:cad>) — where GPX consumers expect them, since \
                     GPX 1.1 has no core element for either. Set false to output plain GPX with \
                     only latitude/longitude/elevation/time. Default true.",
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
    name = "gizza-ai/tcx-to-gpx",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Garmin TCX workout files into GPX, preserving trackpoints and timestamps",
    skill(
        description = "Convert a Garmin Training Center XML (TCX) workout file into a GPX 1.1 document. Each <Activity> (or <Course>) becomes one <trk>, each TCX <Track> becomes one <trkseg>, and each <Trackpoint> with a <Position> becomes a <trkpt lat lon> carrying <ele> (from AltitudeMeters) and <time> (from Time) when present. The Sport attribute becomes the track's <type>, a Course's <Name> (or an Activity's <Id>) becomes the track's <name>, and the earliest trackpoint time becomes <metadata><time>. With include_extensions=true (the default), each trackpoint's heart rate and cadence are emitted as Garmin TrackPointExtension elements (<gpxtpx:hr>/<gpxtpx:cad>), which is where GPX consumers expect them — GPX 1.1 has no core element for either; set include_extensions=false for plain GPX with only latitude/longitude/elevation/time. Trackpoints without a position are skipped (a GPX <trkpt> requires lat/lon). Paste the TCX XML as text. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "tcx-to-gpx", |a: Args| {
            let opt = Options { include_extensions: a.include_extensions };
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
                    "input": { "type": "string", "description": "The TCX (Garmin Training Center XML) document to convert, pasted as text (the contents of a .tcx file — a <TrainingCenterDatabase> with <Activities>/<Activity> or <Courses>/<Course> holding <Track>/<Trackpoint> elements)." },
                    "include_extensions": { "type": "boolean", "default": true, "description": "Emit each trackpoint's heart rate and cadence as Garmin TrackPointExtension elements (<gpxtpx:hr>/<gpxtpx:cad>) — where GPX consumers expect them, since GPX 1.1 has no core element for either. Set false to output plain GPX with only latitude/longitude/elevation/time. Default true." }
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

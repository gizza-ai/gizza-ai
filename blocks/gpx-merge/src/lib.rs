//! gizza-ai/gpx-merge — chat skill block on the shared tool abstraction.
//! Merge multiple GPX files/tracks/routes/waypoints into one GPX 1.1 document,
//! ordered chronologically by timestamp. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_gpx_merge_core::{merge, MergeMode, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_merge_mode")]
    merge_mode: String,
    #[serde(default = "default_true")]
    sort_by_time: bool,
    #[serde(default)]
    dedupe: bool,
    #[serde(default = "default_true")]
    include_waypoints: bool,
    #[serde(default = "default_track_name")]
    track_name: String,
}
fn default_merge_mode() -> String {
    "single-track".to_string()
}
fn default_true() -> bool {
    true
}
fn default_track_name() -> String {
    "Merged track".to_string()
}

/// Turn parsed args into core `Options` (merge_mode string → enum, blank
/// track_name → the default). Shared by handle() and the unit tests.
fn options_from(a: &Args) -> Result<Options, String> {
    let track_name = if a.track_name.trim().is_empty() {
        default_track_name()
    } else {
        a.track_name.clone()
    };
    Ok(Options {
        merge_mode: MergeMode::parse(&a.merge_mode)?,
        sort_by_time: a.sort_by_time,
        dedupe: a.dedupe,
        include_waypoints: a.include_waypoints,
        track_name,
    })
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "One or more GPX documents to merge, pasted as text. Concatenate several \
                     files one after another (each may keep its own <?xml?> and <gpx> root); \
                     every <trk>, <rte>, and <wpt> from every document is collected. Routes \
                     (<rte>/<rtept>) are read as track points. Coordinates, elevation, and \
                     timestamps are preserved verbatim.",
                ),
        )
        .param(
            Param::enumv(
                "merge_mode",
                ["single-track", "single-track-multi-segment", "separate-tracks"],
            )
            .default("single-track")
            .describe(
                "How the merged file is laid out. 'single-track' (default) combines every point \
                 into one continuous <trkseg>. 'single-track-multi-segment' keeps the original \
                 segment breaks (pauses, multi-day stages) as separate <trkseg>s inside one \
                 <trk>. 'separate-tracks' keeps each source track as its own <trk>, preserving \
                 its name.",
            ),
        )
        .param(
            Param::boolean("sort_by_time")
                .default(true)
                .describe(
                    "Order the merged points (single-track), segments (multi-segment), or tracks \
                     (separate-tracks) chronologically by their <time> timestamp. When off, the \
                     input order is preserved. Points with no timestamp keep their input order \
                     and sort after timed points. Default true.",
                ),
        )
        .param(
            Param::boolean("dedupe")
                .default(false)
                .describe(
                    "Drop consecutive duplicate points (identical lat, lon, and time) within \
                     each segment — useful when merged files overlap at their join. Default \
                     false.",
                ),
        )
        .param(
            Param::boolean("include_waypoints")
                .default(true)
                .describe(
                    "Carry <wpt> waypoints (name, desc, sym, elevation, time) from every source \
                     into the merged file. When off, waypoints are dropped and only tracks and \
                     routes are merged. Default true.",
                ),
        )
        .param(
            Param::string("track_name")
                .default("Merged track")
                .describe(
                    "Name for the merged track (single-track modes) and the fallback name for \
                     any unnamed source track (separate-tracks mode). Default \"Merged track\".",
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
    name = "gizza-ai/gpx-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge multiple GPX files into one, ordered chronologically by timestamp",
    skill(
        description = "Merge multiple GPX files, tracks, routes, and waypoints into a single GPX 1.1 document. Paste one or more GPX documents concatenated together (each may keep its own <?xml?>/<gpx> root); every <trk>/<trkseg>/<trkpt>, <rte>/<rtept> (routes are read as track points), and <wpt> is collected regardless of which document it came from. Choose merge_mode: 'single-track' combines every point into one continuous <trkseg>, 'single-track-multi-segment' keeps the original segment breaks (pauses / multi-day stages) as separate <trkseg>s in one <trk>, and 'separate-tracks' keeps each source track as its own <trk> with its name. With sort_by_time on (the default) points/segments/tracks are ordered chronologically by their <time> timestamp (untimed points keep input order and sort last); turn it off to preserve input order. dedupe drops consecutive duplicate points (same lat/lon/time) within a segment — handy where merged files overlap at their join. include_waypoints (default on) carries <wpt> markers into the output. track_name names the merged track (default \"Merged track\"). Coordinates, elevation, and timestamps are preserved verbatim, so precision survives the merge. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gpx-merge", |a: Args| {
            let opt = options_from(&a).map_err(SkillError::InvalidArgs)?;
            merge(&a.input, &opt).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "One or more GPX documents to merge, pasted as text. Concatenate several files one after another (each may keep its own <?xml?> and <gpx> root); every <trk>, <rte>, and <wpt> from every document is collected. Routes (<rte>/<rtept>) are read as track points. Coordinates, elevation, and timestamps are preserved verbatim." },
                    "merge_mode": { "type": "string", "enum": ["single-track", "single-track-multi-segment", "separate-tracks"], "default": "single-track", "description": "How the merged file is laid out. 'single-track' (default) combines every point into one continuous <trkseg>. 'single-track-multi-segment' keeps the original segment breaks (pauses, multi-day stages) as separate <trkseg>s inside one <trk>. 'separate-tracks' keeps each source track as its own <trk>, preserving its name." },
                    "sort_by_time": { "type": "boolean", "default": true, "description": "Order the merged points (single-track), segments (multi-segment), or tracks (separate-tracks) chronologically by their <time> timestamp. When off, the input order is preserved. Points with no timestamp keep their input order and sort after timed points. Default true." },
                    "dedupe": { "type": "boolean", "default": false, "description": "Drop consecutive duplicate points (identical lat, lon, and time) within each segment — useful when merged files overlap at their join. Default false." },
                    "include_waypoints": { "type": "boolean", "default": true, "description": "Carry <wpt> waypoints (name, desc, sym, elevation, time) from every source into the merged file. When off, waypoints are dropped and only tracks and routes are merged. Default true." },
                    "track_name": { "type": "string", "default": "Merged track", "description": "Name for the merged track (single-track modes) and the fallback name for any unnamed source track (separate-tracks mode). Default \"Merged track\"." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn options_defaults_from_minimal_args() {
        let a: Args = serde_json::from_str(r#"{"input":"<gpx/>"}"#).unwrap();
        let o = options_from(&a).unwrap();
        assert_eq!(o.merge_mode, MergeMode::SingleTrack);
        assert!(o.sort_by_time);
        assert!(!o.dedupe);
        assert!(o.include_waypoints);
        assert_eq!(o.track_name, "Merged track");
    }

    #[test]
    fn options_parse_all_params() {
        let a: Args = serde_json::from_str(
            r#"{"input":"x","merge_mode":"separate-tracks","sort_by_time":false,"dedupe":true,"include_waypoints":false,"track_name":"Trip"}"#,
        )
        .unwrap();
        let o = options_from(&a).unwrap();
        assert_eq!(o.merge_mode, MergeMode::SeparateTracks);
        assert!(!o.sort_by_time);
        assert!(o.dedupe);
        assert!(!o.include_waypoints);
        assert_eq!(o.track_name, "Trip");
    }

    #[test]
    fn blank_track_name_falls_back_to_default() {
        let a: Args = serde_json::from_str(r#"{"input":"x","track_name":"   "}"#).unwrap();
        let o = options_from(&a).unwrap();
        assert_eq!(o.track_name, "Merged track");
    }

    #[test]
    fn options_reject_unknown_merge_mode() {
        let a: Args = serde_json::from_str(r#"{"input":"x","merge_mode":"bogus"}"#).unwrap();
        assert!(options_from(&a).is_err());
    }
}

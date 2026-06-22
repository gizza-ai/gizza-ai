//! gizza-ai/calendar-heatmap — render date→value pairs as a GitHub-style year
//! contribution-calendar SVG. Pure-Rust (no deps in core), runs on all backends
//! incl. the chat SW. The SVG is wrapped as image/svg+xml via build_media_envelope
//! (like heatmap-chart / correlation-heatmap). Surfaces: chat + CLI (no page mode
//! for image-bytes output).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_calendar_heatmap_core::render_svg;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    #[serde(default = "default_scheme")]
    scheme: String,
    #[serde(default)]
    start: String,
    #[serde(default)]
    end: String,
    #[serde(default)]
    title: String,
}
fn default_scheme() -> String {
    "green".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The data: one date per line, optionally with a value, as `YYYY-MM-DD` or `YYYY-MM-DD,VALUE` (comma/space/tab separator; value defaults to 1). Repeated dates are summed."))
        .param(Param::enumv("scheme", ["green", "blue", "purple", "orange"]).default("green").describe("Color ramp for the intensity scale. Default green (GitHub-style)."))
        .param(Param::string("start").default("").describe("Optional window start date (YYYY-MM-DD). Defaults to the earliest date in the data."))
        .param(Param::string("end").default("").describe("Optional window end date (YYYY-MM-DD). Defaults to the latest date in the data."))
        .param(Param::string("title").default("").describe("Optional heading drawn above the calendar."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CalendarHeatmap;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/calendar-heatmap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render date/value pairs as a GitHub-style calendar heatmap SVG",
    skill(
        description = "Turn a list of date→value pairs into a GitHub-style year contribution calendar (an SVG): weeks are columns, the seven weekdays are rows, and each day-cell is shaded by its value bucketed into a 5-step intensity scale with a Less→More legend and month/weekday labels. `data` is one date per line as `YYYY-MM-DD` or `YYYY-MM-DD,VALUE` (the value defaults to 1; repeated dates are summed). scheme picks the color ramp (green|blue|purple|orange); start/end optionally fix the date window (defaults to the data's range); title adds a heading. Returns the SVG. Runs locally.",
        parameters = schema_json()
    )
)]
impl CalendarHeatmap {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("calendar-heatmap")?;
    let svg = render_svg(&args.data, &args.scheme, &args.start, &args.end, &args.title)
        .map_err(SkillError::InvalidArgs)?;
    let name = if args.title.trim().is_empty() {
        "calendar-heatmap".to_string()
    } else {
        args.title.trim().replace(['/', '\\', ' '], "-")
    };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{name}.svg"),
        format!("rendered a calendar heatmap ({} bytes SVG)", svg.len()),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":   { "type": "string", "description": "The data: one date per line, optionally with a value, as `YYYY-MM-DD` or `YYYY-MM-DD,VALUE` (comma/space/tab separator; value defaults to 1). Repeated dates are summed." },
                    "scheme": { "type": "string", "enum": ["green", "blue", "purple", "orange"], "default": "green", "description": "Color ramp for the intensity scale. Default green (GitHub-style)." },
                    "start":  { "type": "string", "default": "", "description": "Optional window start date (YYYY-MM-DD). Defaults to the earliest date in the data." },
                    "end":    { "type": "string", "default": "", "description": "Optional window end date (YYYY-MM-DD). Defaults to the latest date in the data." },
                    "title":  { "type": "string", "default": "", "description": "Optional heading drawn above the calendar." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

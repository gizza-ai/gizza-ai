//! gizza-ai/org-chart — render an organizational chart image from an indented
//! outline or JSON of people and managers.
//!
//! Pure-Rust (no deps in core), so it runs on ALL backends including the chat
//! Service Worker. The SVG is wrapped as an `image/svg+xml` data-URL envelope so
//! the chat UI shows the chart. Surfaces: chat + CLI (no standalone page — a
//! pure-Rust image-bytes output has no page mode, like scatter-chart).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_org_chart_core::render_svg;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_color")]
    color: String,
}
fn default_direction() -> String {
    "vertical".to_string()
}
fn default_color() -> String {
    "#4e79a7".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The people to chart, in either format. (1) Indented outline: one person per line, leading spaces or tabs set who reports to whom; add a title/department after `|`, e.g. `CEO | Chief Exec`, then `  VP Eng | VP Engineering | Product`. (2) JSON: nested `{\"name\":..,\"title\":..,\"department\":..,\"children\":[..]}` (object or array of roots), or a flat list `[{\"name\":\"Ada\",\"title\":\"CEO\"},{\"name\":\"Grace\",\"manager\":\"Ada\"}]` linked by manager name. Max 400 people.",
        ))
        .param(Param::string("title").default("").describe("Optional heading shown above the chart (e.g. \"Engineering Org\"). Empty for none."))
        .param(
            Param::enumv("direction", ["vertical", "horizontal"])
                .default("vertical")
                .describe("Layout direction: \"vertical\" (top-down, the boss on top) or \"horizontal\" (left-to-right). Default \"vertical\"."),
        )
        .param(Param::string("color").default("#4e79a7").describe(
            "Accent colour for each box's top bar and border — any CSS colour (hex like #336699, rgb(), or a named colour). Default #4e79a7.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct OrgChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/org-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render an organizational chart as an SVG image",
    skill(
        description = "Render an organizational chart from an indented outline or JSON of people and managers into an SVG image. Input `data` is either an indented outline (one person per line; leading spaces/tabs set reporting depth; optional `Name | Title | Department`) or JSON (nested `{name,title,department,children}`, or a flat list linked by `manager`). Options: `title` heading, `direction` (vertical top-down or horizontal left-to-right), and `color` accent. Returns an SVG image of the hierarchy. Max 400 people.",
        parameters = schema_json()
    ),
)]
impl OrgChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("org-chart")?;
    let svg = render_svg(&args.data, &args.title, &args.direction, &args.color)
        .map_err(SkillError::InvalidArgs)?;
    let name = if args.title.trim().is_empty() {
        "org-chart".to_string()
    } else {
        args.title.trim().to_string()
    };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{}.svg", name.replace(['/', '\\', ' '], "-")),
        format!("rendered an org chart ({} bytes SVG)", svg.len()),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The people to chart, in either format. (1) Indented outline: one person per line, leading spaces or tabs set who reports to whom; add a title/department after `|`, e.g. `CEO | Chief Exec`, then `  VP Eng | VP Engineering | Product`. (2) JSON: nested `{\"name\":..,\"title\":..,\"department\":..,\"children\":[..]}` (object or array of roots), or a flat list `[{\"name\":\"Ada\",\"title\":\"CEO\"},{\"name\":\"Grace\",\"manager\":\"Ada\"}]` linked by manager name. Max 400 people." },
                    "title": { "type": "string", "default": "", "description": "Optional heading shown above the chart (e.g. \"Engineering Org\"). Empty for none." },
                    "direction": { "type": "string", "enum": ["vertical", "horizontal"], "default": "vertical", "description": "Layout direction: \"vertical\" (top-down, the boss on top) or \"horizontal\" (left-to-right). Default \"vertical\"." },
                    "color": { "type": "string", "default": "#4e79a7", "description": "Accent colour for each box's top bar and border — any CSS colour (hex like #336699, rgb(), or a named colour). Default #4e79a7." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

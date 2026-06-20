//! gizza-ai/correlation-heatmap — correlation matrix → SVG heatmap.
//! Pure-Rust (no deps in core), so it runs on all backends incl. the chat SW.
//! The SVG is wrapped as image/svg+xml via build_media_envelope (like vectorize/
//! line-series-chart). Surfaces: chat + CLI (no page mode for image-bytes out).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_correlation_heatmap_core::{render_svg, Method};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    title: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("Rows of numbers (CSV-like): each column is a variable, each row an observation. At least 2 columns and 2 rows."))
        .param(Param::enumv("method", ["pearson", "spearman"]).default("pearson").describe("Correlation method: pearson (linear, default) or spearman (rank/monotonic)."))
        .param(Param::string("labels").default("").describe("Optional comma-separated column names (defaults to v1..vN)."))
        .param(Param::string("title").default("").describe("Optional chart title."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CorrelationHeatmap;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/correlation-heatmap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Correlation matrix as an SVG heatmap",
    skill(
        description = "Compute a Pearson or Spearman correlation matrix across numeric columns and render it as a labeled SVG heatmap (diverging blue→white→red color scale with the correlation value in each cell). `data` is rows of numbers (each column a variable, each row an observation); method is pearson (default) or spearman; labels optionally names the columns.",
        parameters = schema_json()
    )
)]
impl CorrelationHeatmap {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("correlation-heatmap")?;
    let method = Method::parse(&args.method).map_err(SkillError::InvalidArgs)?;
    let svg = render_svg(&args.data, method, &args.labels, &args.title).map_err(SkillError::InvalidArgs)?;
    let name = if args.title.is_empty() { "correlation".to_string() } else { args.title.replace(['/', '\\', ' '], "-") };
    build_media_envelope(svg.as_bytes(), "image/svg+xml", format!("{name}.svg"), format!("rendered a correlation heatmap ({} bytes SVG)", svg.len()), MAX_OUTPUT_BYTES)
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
                    "data":   { "type": "string", "description": "Rows of numbers (CSV-like): each column is a variable, each row an observation. At least 2 columns and 2 rows." },
                    "method": { "type": "string", "enum": ["pearson", "spearman"], "default": "pearson", "description": "Correlation method: pearson (linear, default) or spearman (rank/monotonic)." },
                    "labels": { "type": "string", "default": "", "description": "Optional comma-separated column names (defaults to v1..vN)." },
                    "title":  { "type": "string", "default": "", "description": "Optional chart title." }
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

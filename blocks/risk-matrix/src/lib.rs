//! gizza-ai/risk-matrix — render a likelihood×impact risk matrix as an SVG image.
//! Pure-Rust (no deps in core), runs on all backends incl. the chat SW. The SVG is
//! wrapped as image/svg+xml via build_media_envelope (like heatmap-chart /
//! correlation-heatmap). Surfaces: chat + CLI (no page mode for image-bytes out).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_risk_matrix_core::render_svg;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    items: String,
    #[serde(default = "default_size")]
    size: f64,
    #[serde(default)]
    likelihood_labels: String,
    #[serde(default)]
    impact_labels: String,
    #[serde(default = "default_amber")]
    amber_at: f64,
    #[serde(default = "default_red")]
    red_at: f64,
    #[serde(default)]
    title: String,
}
fn default_size() -> f64 {
    5.0
}
fn default_amber() -> f64 {
    0.25
}
fn default_red() -> f64 {
    0.5
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("items").required().describe(
            "The risk register: one item per line as `name, likelihood, impact`. likelihood and impact are integers 1..=size (the name may contain commas). E.g. `Server outage, 4, 5`.",
        ))
        .param(Param::integer("size").default(5).min(2.0).max(10.0).describe(
            "Matrix dimension N (an N×N grid, likelihood and impact each rated 1..=N). Default 5.",
        ))
        .param(Param::string("likelihood_labels").default("").describe(
            "Optional comma-separated names for the likelihood (X) axis, low→high (defaults to 1..N).",
        ))
        .param(Param::string("impact_labels").default("").describe(
            "Optional comma-separated names for the impact (Y) axis, low→high (defaults to 1..N).",
        ))
        .param(Param::number("amber_at").default(0.25).describe(
            "Fraction of the max score (size×size) at/below which a cell is green/Low. Default 0.25.",
        ))
        .param(Param::number("red_at").default(0.5).describe(
            "Fraction of the max score at/below which a cell is amber/Medium (above it is red/High). Must exceed amber_at. Default 0.5.",
        ))
        .param(Param::string("title").default("").describe("Optional chart title."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RiskMatrix;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/risk-matrix",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Plot likelihood-vs-impact items onto a colored risk matrix",
    skill(
        description = "Plot likelihood-versus-impact items onto a classic colored risk-matrix heatmap (green/amber/red probability-impact grid). `items` is a risk register, one per line as `name, likelihood, impact` with integer ratings 1..=size. Each cell is shaded by its risk score (likelihood × impact) relative to the grid maximum — amber_at and red_at set the Low/Medium/High band fractions — and items are drawn as numbered markers in their cell with a legend listing name, L×I = score and band. Likelihood is the X axis, impact the Y axis (High-risk corner top-right). Returns an SVG image.",
        parameters = schema_json()
    )
)]
impl RiskMatrix {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("risk-matrix")?;
    let svg = render_svg(
        &args.items,
        args.size.round() as usize,
        &args.likelihood_labels,
        &args.impact_labels,
        args.amber_at,
        args.red_at,
        &args.title,
    )
    .map_err(SkillError::InvalidArgs)?;
    let name = if args.title.is_empty() {
        "risk-matrix".to_string()
    } else {
        args.title.replace(['/', '\\', ' '], "-")
    };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("{name}.svg"),
        format!("rendered a risk matrix ({} bytes SVG)", svg.len()),
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
                    "items":             { "type": "string", "description": "The risk register: one item per line as `name, likelihood, impact`. likelihood and impact are integers 1..=size (the name may contain commas). E.g. `Server outage, 4, 5`." },
                    "size":              { "type": "integer", "default": 5, "minimum": 2, "maximum": 10, "description": "Matrix dimension N (an N×N grid, likelihood and impact each rated 1..=N). Default 5." },
                    "likelihood_labels": { "type": "string", "default": "", "description": "Optional comma-separated names for the likelihood (X) axis, low→high (defaults to 1..N)." },
                    "impact_labels":     { "type": "string", "default": "", "description": "Optional comma-separated names for the impact (Y) axis, low→high (defaults to 1..N)." },
                    "amber_at":          { "type": "number", "default": 0.25, "description": "Fraction of the max score (size×size) at/below which a cell is green/Low. Default 0.25." },
                    "red_at":            { "type": "number", "default": 0.5, "description": "Fraction of the max score at/below which a cell is amber/Medium (above it is red/High). Must exceed amber_at. Default 0.5." },
                    "title":             { "type": "string", "default": "", "description": "Optional chart title." }
                },
                "required": ["items"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

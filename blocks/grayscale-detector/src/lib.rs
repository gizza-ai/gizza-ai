//! gizza-ai/grayscale-detector — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_base64")]
    input_format: String,
    #[serde(default = "default_tolerance")]
    tolerance: f64,
    #[serde(default = "default_metric")]
    metric: String,
    #[serde(default = "default_true")]
    ignore_alpha: bool,
    #[serde(default = "default_max_samples")]
    max_samples: f64,
    #[serde(default = "default_report")]
    output: String,
}

fn default_base64() -> String {
    "base64".to_string()
}
fn default_metric() -> String {
    "channel_delta".to_string()
}
fn default_report() -> String {
    "report".to_string()
}
fn default_tolerance() -> f64 {
    2.0
}
fn default_max_samples() -> f64 {
    20.0
}
fn default_true() -> bool {
    true
}

fn whole_number(n: f64, field: &str, hi: f64) -> Result<u32, SkillError> {
    if !n.is_finite() || n.fract() != 0.0 || !(0.0..=hi).contains(&n) {
        return Err(SkillError::InvalidArgs(format!(
            "{field} must be a whole number from 0 to {hi:.0} (got {n})"
        )));
    }
    Ok(n as u32)
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe(
            "Image file bytes encoded as base64 or hex. Supports PNG, JPEG, WebP, GIF, BMP, and TIFF. Max 32 MiB decoded.",
        ))
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe(
                    "Encoding of `input`: base64 (default, standard or URL-safe alphabet) or hex (whitespace and ':' '-' '_' separators are ignored).",
                ),
        )
        .param(
            Param::integer("tolerance")
                .default(2)
                .min(0.0)
                .max(255.0)
                .describe(
                    "Highest per-pixel colorfulness score (0-255) still counted as gray. Use 0 for strict R=G=B; the default 2 absorbs JPEG/WebP compression noise; 255 accepts every pixel.",
                ),
        )
        .param(
            Param::enumv("metric", ["channel_delta", "saturation"])
                .default("channel_delta")
                .describe(
                    "How each pixel is scored: channel_delta (default) = max minus min of R, G, B; saturation = HSV saturation on a 0-255 scale, which also catches faint tints in dark pixels.",
                ),
        )
        .param(Param::boolean("ignore_alpha").default(true).describe(
            "true (default) scores every pixel on its RGB channels regardless of transparency. false excludes fully transparent pixels (alpha 0) from the verdict and reports them separately.",
        ))
        .param(
            Param::integer("max_samples")
                .default(20)
                .min(0.0)
                .max(200.0)
                .describe(
                    "How many example color pixels to list with their coordinates and hex value (0-200, default 20). Use 0 to report counts only.",
                ),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe(
                    "Output format: report (default, human-readable verdict with counts and sample pixels) or json (structured metrics and sample coordinates).",
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
    name = "gizza-ai/grayscale-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect whether an RGB image's pixels are effectively grayscale",
    skill(
        description = "Detect whether an image stored as RGB is effectively grayscale by scoring every pixel's red/green/blue spread. Paste image bytes as base64 or hex; PNG, JPEG, WebP, GIF, BMP and TIFF are supported, up to 32 MiB decoded. Options: tolerance 0-255 (default 2) for the highest score still counted as gray, metric channel_delta or saturation, ignore_alpha true/false (default true), max_samples 0-200 (default 20), and output report or json. The result gives a verdict plus dimensions, scanned/gray/color pixel counts and percentages, max and mean score, and sample color-pixel coordinates with hex values.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "grayscale-detector", |a: Args| {
            gizza_ai_grayscale_detector_core::run(
                &a.input,
                &a.input_format,
                whole_number(a.tolerance, "tolerance", 255.0)? as u8,
                &a.metric,
                a.ignore_alpha,
                whole_number(a.max_samples, "max_samples", 200.0)?,
                &a.output,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["required"], serde_json::json!(["input"]));
        assert_eq!(v["properties"]["input"]["type"], "string");
        assert_eq!(
            v["properties"]["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(v["properties"]["input_format"]["default"], "base64");
        assert_eq!(v["properties"]["tolerance"]["type"], "integer");
        assert_eq!(v["properties"]["tolerance"]["default"], 2);
        assert_eq!(v["properties"]["tolerance"]["minimum"], 0.0);
        assert_eq!(v["properties"]["tolerance"]["maximum"], 255.0);
        assert_eq!(
            v["properties"]["metric"]["enum"],
            serde_json::json!(["channel_delta", "saturation"])
        );
        assert_eq!(v["properties"]["metric"]["default"], "channel_delta");
        assert_eq!(v["properties"]["ignore_alpha"]["type"], "boolean");
        assert_eq!(v["properties"]["ignore_alpha"]["default"], true);
        assert_eq!(v["properties"]["max_samples"]["default"], 20);
        assert_eq!(v["properties"]["max_samples"]["minimum"], 0.0);
        assert_eq!(v["properties"]["max_samples"]["maximum"], 200.0);
        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["report", "json"])
        );
        assert_eq!(v["properties"]["output"]["default"], "report");
        assert_eq!(v["additionalProperties"], false);
        for p in [
            "input",
            "input_format",
            "tolerance",
            "metric",
            "ignore_alpha",
            "max_samples",
            "output",
        ] {
            assert!(
                v["properties"][p]["description"]
                    .as_str()
                    .is_some_and(|d| !d.is_empty()),
                "{p} is missing a description"
            );
        }
    }

    #[test]
    fn whole_number_bounds() {
        assert_eq!(whole_number(0.0, "tolerance", 255.0).unwrap(), 0);
        assert_eq!(whole_number(255.0, "tolerance", 255.0).unwrap(), 255);
        assert!(matches!(
            whole_number(1.5, "tolerance", 255.0),
            Err(SkillError::InvalidArgs(_))
        ));
        assert!(matches!(
            whole_number(256.0, "tolerance", 255.0),
            Err(SkillError::InvalidArgs(_))
        ));
        assert!(matches!(
            whole_number(201.0, "max_samples", 200.0),
            Err(SkillError::InvalidArgs(_))
        ));
    }
}

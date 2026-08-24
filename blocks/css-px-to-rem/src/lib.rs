//! gizza-ai/css-px-to-rem — rewrite the px lengths in a CSS stylesheet to rem
//! (or rem back to px) against a configurable root font size. Chat schema
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_css_px_to_rem_core::{convert, Direction, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    css: String,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default = "default_root_font_size")]
    root_font_size: f64,
    #[serde(default = "default_precision")]
    precision: u64,
    #[serde(default = "default_properties")]
    properties: String,
    #[serde(default)]
    min_pixel_value: f64,
    #[serde(default)]
    media_queries: bool,
    #[serde(default)]
    ignore_selectors: String,
    #[serde(default)]
    keep_fallback: bool,
    #[serde(default = "default_unitless_zero")]
    unitless_zero: bool,
}

fn default_root_font_size() -> f64 {
    16.0
}
fn default_precision() -> u64 {
    5
}
fn default_properties() -> String {
    "*".to_string()
}
fn default_unitless_zero() -> bool {
    true
}

/// Resolve [`Args`] into core [`Options`], validating the enum param.
fn resolve(a: &Args) -> Result<Options, String> {
    Ok(Options {
        direction: Direction::parse(a.direction.as_deref())?,
        root_font_size: a.root_font_size,
        precision: a.precision as usize,
        properties: a.properties.clone(),
        min_pixel_value: a.min_pixel_value,
        media_queries: a.media_queries,
        ignore_selectors: a.ignore_selectors.clone(),
        keep_fallback: a.keep_fallback,
        unitless_zero: a.unitless_zero,
    })
}

fn rewrite(a: &Args) -> Result<String, String> {
    convert(&a.css, &resolve(a)?)
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("css")
                .required()
                .describe("The CSS (or SCSS/LESS) source to rewrite, e.g. \".btn { font-size: 24px; padding: 8px 16px; }\". Comments, quoted strings and url(...) payloads are never touched."),
        )
        .param(
            Param::enumv("direction", ["px-to-rem", "rem-to-px"])
                .default("px-to-rem")
                .describe("Which way to convert: 'px-to-rem' divides each px length by root_font_size, 'rem-to-px' multiplies each rem length by it. Default px-to-rem."),
        )
        .param(
            Param::number("root_font_size")
                .min(1.0)
                .max(100.0)
                .default(16.0)
                .describe("The root (html) font size in px that 1rem stands for, 1-100. Browsers default to 16; use 10 for the 62.5% (html { font-size: 62.5% }) trick. Default 16."),
        )
        .param(
            Param::integer("precision")
                .min(0.0)
                .max(10.0)
                .default(5)
                .describe("Decimal places kept on each converted number, 0-10. Trailing zeros are always trimmed (0.5000 → 0.5). Default 5."),
        )
        .param(
            Param::string("properties")
                .default("*")
                .describe("Comma-separated list of CSS properties to convert, with wildcards: '*' = all (default), 'font*' = prefix, '*width' = suffix, '*margin*' = contains, and a leading '!' excludes (e.g. '*,!border*' converts everything except border properties)."),
        )
        .param(
            Param::number("min_pixel_value")
                .min(0.0)
                .default(0.0)
                .describe("Leave lengths whose px magnitude is below this untouched — set 2 to keep 1px hairline borders in px. Applies to the px side in both directions. Default 0 (convert everything)."),
        )
        .param(
            Param::boolean("media_queries")
                .default(false)
                .describe("Also convert lengths inside @media conditions (e.g. @media (min-width: 640px)). Default false, since breakpoints are usually kept in px."),
        )
        .param(
            Param::string("ignore_selectors")
                .describe("Comma-separated substrings; any rule whose selector (or enclosing selector) contains one is left entirely in its original units, e.g. \".no-rem, #legacy\". Empty by default."),
        )
        .param(
            Param::boolean("keep_fallback")
                .default(false)
                .describe("Keep the original declaration and append the converted one after it (font-size: 16px; font-size: 1rem;) as a fallback pair, instead of replacing it. Default false."),
        )
        .param(
            Param::boolean("unitless_zero")
                .default(true)
                .describe("Write a zero-valued length as a bare 0 rather than 0rem/0px. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/css-px-to-rem",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert px lengths in CSS to rem (or back) at any root font size",
    skill(
        description = "Rewrite a whole CSS/SCSS/LESS stylesheet's px lengths to rem against a configurable root font size — or rem back to px with direction='rem-to-px'. root_font_size (default 16; use 10 for the 62.5% trick) sets what 1rem means; precision (0-10, default 5) caps decimals and trailing zeros are trimmed; properties filters which declarations convert with '*' wildcards and '!' exclusions (default '*' = all); min_pixel_value keeps small values such as 1px borders in px; media_queries (default false) opts breakpoint conditions in; ignore_selectors skips matching rules; keep_fallback emits px and rem declarations as a pair; unitless_zero (default true) writes 0 instead of 0rem. Comments, quoted strings, url(...) payloads and identifiers are preserved byte for byte, and a capitalized unit (16Px) opts a single value out. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "css-px-to-rem", |a: Args| {
            rewrite(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(css: &str) -> Args {
        Args {
            css: css.to_string(),
            direction: None,
            root_font_size: default_root_font_size(),
            precision: default_precision(),
            properties: default_properties(),
            min_pixel_value: 0.0,
            media_queries: false,
            ignore_selectors: String::new(),
            keep_fallback: false,
            unitless_zero: default_unitless_zero(),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "css":              { "type": "string", "description": "The CSS (or SCSS/LESS) source to rewrite, e.g. \".btn { font-size: 24px; padding: 8px 16px; }\". Comments, quoted strings and url(...) payloads are never touched." },
                    "direction":        { "type": "string", "enum": ["px-to-rem", "rem-to-px"], "default": "px-to-rem", "description": "Which way to convert: 'px-to-rem' divides each px length by root_font_size, 'rem-to-px' multiplies each rem length by it. Default px-to-rem." },
                    "root_font_size":   { "type": "number", "minimum": 1, "maximum": 100, "default": 16.0, "description": "The root (html) font size in px that 1rem stands for, 1-100. Browsers default to 16; use 10 for the 62.5% (html { font-size: 62.5% }) trick. Default 16." },
                    "precision":        { "type": "integer", "minimum": 0, "maximum": 10, "default": 5, "description": "Decimal places kept on each converted number, 0-10. Trailing zeros are always trimmed (0.5000 → 0.5). Default 5." },
                    "properties":       { "type": "string", "default": "*", "description": "Comma-separated list of CSS properties to convert, with wildcards: '*' = all (default), 'font*' = prefix, '*width' = suffix, '*margin*' = contains, and a leading '!' excludes (e.g. '*,!border*' converts everything except border properties)." },
                    "min_pixel_value":  { "type": "number", "minimum": 0, "default": 0.0, "description": "Leave lengths whose px magnitude is below this untouched — set 2 to keep 1px hairline borders in px. Applies to the px side in both directions. Default 0 (convert everything)." },
                    "media_queries":    { "type": "boolean", "default": false, "description": "Also convert lengths inside @media conditions (e.g. @media (min-width: 640px)). Default false, since breakpoints are usually kept in px." },
                    "ignore_selectors": { "type": "string", "description": "Comma-separated substrings; any rule whose selector (or enclosing selector) contains one is left entirely in its original units, e.g. \".no-rem, #legacy\". Empty by default." },
                    "keep_fallback":    { "type": "boolean", "default": false, "description": "Keep the original declaration and append the converted one after it (font-size: 16px; font-size: 1rem;) as a fallback pair, instead of replacing it. Default false." },
                    "unitless_zero":    { "type": "boolean", "default": true, "description": "Write a zero-valued length as a bare 0 rather than 0rem/0px. Default true." }
                },
                "required": ["css"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn rewrite_defaults_convert_px_to_rem() {
        assert_eq!(
            rewrite(&args(".btn{font-size:24px;padding:8px 0px}")).unwrap(),
            ".btn{font-size:1.5rem;padding:0.5rem 0}"
        );
    }

    #[test]
    fn rewrite_rejects_an_unknown_direction() {
        let mut a = args("a{width:16px}");
        a.direction = Some("px-to-em".to_string());
        let err = rewrite(&a).unwrap_err();
        assert!(err.contains("invalid direction"), "got: {err}");
    }

    #[test]
    fn rewrite_rejects_a_zero_root_font_size() {
        let mut a = args("a{width:16px}");
        a.root_font_size = 0.0;
        let err = rewrite(&a).unwrap_err();
        assert!(err.contains("invalid root_font_size"), "got: {err}");
    }
}

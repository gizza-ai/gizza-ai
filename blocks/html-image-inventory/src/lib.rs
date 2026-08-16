//! gizza-ai/html-image-inventory — build a table of every image source in
//! pasted HTML (`<img>` plus every `<picture><source>` candidate) with its alt
//! text, width/height, and loading/decoding hints, flagging the images that are
//! missing alt text or explicit dimensions. Thin wrapper around the core; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_image_inventory_core::{inventory, parse_format, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    format: String,
    #[serde(default = "default_true")]
    include_sources: bool,
    #[serde(default)]
    only_issues: bool,
    #[serde(default)]
    flag_empty_alt: bool,
    #[serde(default = "default_true")]
    include_summary: bool,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("The raw HTML to inventory, e.g. a page's source, a template fragment, or an email body. Parsed with a real HTML parser, so unquoted attributes and unclosed tags are fine."))
        .param(Param::enumv("format", ["markdown", "csv", "json"]).default("markdown").describe("Output shape: markdown (default, one table row per image plus a responsive-sources list), csv (one flat row per image, spreadsheet-ready), or json (structured, with a counts summary)."))
        .param(Param::boolean("include_sources").default(true).describe("Include each <picture><source> candidate as its own row, with its srcset, media query, and type. Default true. <source> elements inside <video>/<audio> are always ignored."))
        .param(Param::boolean("only_issues").default(false).describe("List only the rows that carry at least one issue (missing-alt, missing-width, missing-height, no-source). Default false — list every image."))
        .param(Param::boolean("flag_empty_alt").default(false).describe("Also flag images with an explicit alt=\"\" as an issue. Default false, because an empty alt is the correct markup for a purely decorative image."))
        .param(Param::boolean("include_summary").default(true).describe("Prepend the counts summary (images, picture sources, missing alt, missing dimensions, lazy-loaded). Default true; set false for a bare table."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlImageInventory;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-image-inventory",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Table every img and picture source in HTML with alt text, size, and loading hints",
    skill(
        description = "Build a table of every image source declared in pasted HTML. Each <img> and each <picture><source> candidate becomes a row reporting src, srcset, sizes, alt text, width, height, loading, decoding, fetchpriority, media, type, class, id, and title. Images are flagged missing-alt (no alt attribute at all — an explicit alt=\"\" is the correct decorative marker and is not flagged unless flag_empty_alt=true), missing-width / missing-height (no valid non-negative-integer content attribute, so the browser reserves no space and the layout shifts as the image loads), and no-source (neither src nor srcset). format='markdown' (default), 'csv', or 'json'. include_sources (default true) adds the <picture><source> rows; only_issues (default false) lists just the flagged rows; include_summary (default true) prepends the counts. Attributes are read from the static markup only: nothing is fetched, so real file sizes, real pixel dimensions, broken links, CSS background images, and JavaScript-injected images are out of scope. Caps at 2000 rows.",
        parameters = schema_json()
    )
)]
impl HtmlImageInventory {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-image-inventory", |a: Args| {
            let fmt = parse_format(&a.format).map_err(SkillError::InvalidArgs)?;
            inventory(
                &a.html,
                fmt,
                &Options {
                    include_sources: a.include_sources,
                    only_issues: a.only_issues,
                    flag_empty_alt: a.flag_empty_alt,
                    include_summary: a.include_summary,
                },
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "html":            { "type": "string", "description": "The raw HTML to inventory, e.g. a page's source, a template fragment, or an email body. Parsed with a real HTML parser, so unquoted attributes and unclosed tags are fine." },
                    "format":          { "type": "string", "enum": ["markdown", "csv", "json"], "default": "markdown", "description": "Output shape: markdown (default, one table row per image plus a responsive-sources list), csv (one flat row per image, spreadsheet-ready), or json (structured, with a counts summary)." },
                    "include_sources": { "type": "boolean", "default": true, "description": "Include each <picture><source> candidate as its own row, with its srcset, media query, and type. Default true. <source> elements inside <video>/<audio> are always ignored." },
                    "only_issues":     { "type": "boolean", "default": false, "description": "List only the rows that carry at least one issue (missing-alt, missing-width, missing-height, no-source). Default false — list every image." },
                    "flag_empty_alt":  { "type": "boolean", "default": false, "description": "Also flag images with an explicit alt=\"\" as an issue. Default false, because an empty alt is the correct markup for a purely decorative image." },
                    "include_summary": { "type": "boolean", "default": true, "description": "Prepend the counts summary (images, picture sources, missing alt, missing dimensions, lazy-loaded). Default true; set false for a bare table." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

//! gizza-ai/font-subset — subset an OpenType font to the glyphs used by text,
//! returning a downloadable smaller font.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    text: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    drop_variations: bool,
}

fn default_format() -> String {
    "woff2".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::string("text")
                .required()
                .describe("Text whose characters should be retained in the subset font. Control characters are ignored; at least one printable covered character is required."),
        )
        .param(
            Param::enumv("format", ["woff2", "opentype"])
                .default("woff2")
                .describe("Output font container: woff2 for a compressed webfont (default) or opentype for raw SFNT/OpenType bytes. The input must be a single TTF/OTF/OpenType font, not WOFF/WOFF2 or a font collection."),
        )
        .param(
            Param::boolean("drop_variations")
                .default(false)
                .describe("Drop variable-font tables before subsetting. This can shrink variable fonts further, but loses variation axes. Off by default."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FontSubset;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/font-subset",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Subset an OpenType font to the glyphs used by text",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Subset a single OpenType/TTF/OTF font file to only the glyphs needed by the supplied text, returning a smaller downloadable font. Provide the source font as url (HTTP/HTTPS) or ref (id from a prior tool call). text is the sample text whose printable characters are retained. format is woff2 (default, compressed webfont) or opentype (raw SFNT/OpenType bytes). drop_variations optionally removes variable-font tables before subsetting. WOFF/WOFF2 input and font collections (.ttc/.otc) are not accepted; convert webfont containers to OpenType first with woff2-convert.",
        parameters = schema_json()
    ),
)]
impl FontSubset {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn output_name(in_filename: &str, ext: &str) -> String {
    let stem = in_filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .filter(|s| !s.is_empty())
        .unwrap_or(in_filename)
        .trim();
    let stem = if stem.is_empty() { "font" } else { stem };
    format!("{stem}.subset.{ext}")
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("font-subset")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let format = gizza_ai_font_subset_core::OutputFormat::parse(&args.format)
        .map_err(SkillError::InvalidArgs)?;
    let res = gizza_ai_font_subset_core::subset(&bytes, &args.text, format, args.drop_variations)
        .map_err(SkillError::InvalidArgs)?;

    let filename = output_name(&in_filename, res.format.ext());
    let missing = if res.missing_chars.is_empty() {
        "all requested characters were present".to_string()
    } else {
        let preview: String = res.missing_chars.iter().take(12).collect();
        format!(
            "{} requested characters were missing from the font (first: {preview})",
            res.missing_chars.len()
        )
    };
    let savings = if res.output_size < res.input_size {
        format!(
            "{}% smaller",
            100 - (res.output_size * 100 / res.input_size.max(1))
        )
    } else {
        format!(
            "{:.1}x of original size",
            res.output_size as f64 / res.input_size.max(1) as f64
        )
    };
    let for_llm = format!(
        "subset font from {} glyphs to {} requested characters as {}: {filename} ({} → {} bytes, {savings}); {missing}",
        res.input_glyphs,
        res.kept_chars,
        res.format.label(),
        res.input_size,
        res.output_size
    );
    let mime = res.format.mime().to_string();
    let data_url = format!("data:{mime};base64,{}", B64.encode(&res.bytes));
    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime,
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
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
                    "url":             { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":             { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "text":            { "type": "string", "description": "Text whose characters should be retained in the subset font. Control characters are ignored; at least one printable covered character is required." },
                    "format":          { "type": "string", "enum": ["woff2", "opentype"], "default": "woff2", "description": "Output font container: woff2 for a compressed webfont (default) or opentype for raw SFNT/OpenType bytes. The input must be a single TTF/OTF/OpenType font, not WOFF/WOFF2 or a font collection." },
                    "drop_variations": { "type": "boolean", "default": false, "description": "Drop variable-font tables before subsetting. This can shrink variable fonts further, but loses variation axes. Off by default." }
                },
                "required": ["text"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

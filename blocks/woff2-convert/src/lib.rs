//! gizza-ai/woff2-convert — convert a font between the TTF, OTF, WOFF and WOFF2
//! container formats, returned as a downloadable file.
//!
//! The input container is auto-detected from its leading bytes. WOFF/WOFF2 inputs
//! are decoded back to SFNT (`wuff`) and the SFNT is re-encoded to the requested
//! target: WOFF2 (`ttf2woff2` glyf transform for TrueType, a null-transform Brotli
//! writer for CFF/OTF), WOFF v1 (per-table zlib), or raw SFNT (ttf/otf — decompress
//! a web font to a desktop font). The glyph outline technology (TrueType `glyf` vs
//! PostScript/CFF) is PRESERVED — this is a container conversion, not glyph
//! re-outlining.
//!
//! Pure Rust (byte-slice in/out) → runs on ALL backends including the chat Service
//! Worker. Surfaces: chat + CLI. No standalone page (binary font in / font out, the
//! no-page file-input pattern, like file-compressor / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

// Fonts are small; cap well under the sandbox limit so best-quality Brotli on a
// large face stays within memory/time budget.
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    format: String,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `format` (a fixed enum) is the
/// target container.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::enumv("format", ["woff2", "woff", "ttf", "otf"])
            .required()
            .describe(
                "Target font container: woff2 (best-compressed web font, Brotli), \
                 woff (older web font, zlib), ttf or otf (uncompressed desktop \
                 font). The input format (TTF/OTF/WOFF/WOFF2) is auto-detected. \
                 The glyph outline technology is preserved — ttf↔otf changes the \
                 container/extension, it does not re-outline glyphs.",
            ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Woff2Convert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/woff2-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a font between TTF, OTF, WOFF and WOFF2",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert a font file between the TTF, OTF, WOFF and WOFF2 container formats, returned for download. The input format is auto-detected from the file's bytes. format is the target: woff2 (smallest web font, Brotli — best for the web), woff (older web font, zlib), ttf or otf (uncompressed desktop font, i.e. decompress a web font). WOFF2 uses the glyf/loca transform for TrueType fonts and a null-transform Brotli container for CFF/OpenType fonts. The glyph outline technology (TrueType or PostScript/CFF) is preserved — ttf↔otf changes the container/extension, it does NOT re-outline glyphs. Font collections (.ttc), and legacy inputs (EOT, SVG, Type1) are not supported. Provide the font as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl Woff2Convert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// CSS `format()` hint for an `@font-face src`.
#[cfg(target_arch = "wasm32")]
fn css_format_hint(ext: &str) -> &'static str {
    match ext {
        "woff2" => "woff2",
        "woff" => "woff",
        "otf" => "opentype",
        _ => "truetype",
    }
}

/// Rename `input.<oldext>` → `input.<newext>`; fall back to `font.<newext>`.
#[cfg(target_arch = "wasm32")]
fn output_name(in_filename: &str, ext: &str) -> String {
    let stem = in_filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    if stem.is_empty() {
        format!("font.{ext}")
    } else {
        format!("{stem}.{ext}")
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("woff2-convert")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let c = gizza_ai_woff2_convert_core::convert(&bytes, &args.format)
        .map_err(SkillError::InvalidArgs)?;

    let base = if in_filename.is_empty() { "font".to_string() } else { in_filename };
    let filename = output_name(&base, c.ext);
    let mime = c.mime.to_string();

    let delta = if c.output_size < c.input_size {
        format!("{}% smaller", 100 - (c.output_size * 100 / c.input_size.max(1)))
    } else if c.output_size > c.input_size {
        format!("{:.1}x larger", c.output_size as f64 / c.input_size as f64)
    } else {
        "same size".to_string()
    };

    let mut for_llm = format!(
        "converted {} font ({} bytes, {}) → {filename} ({} bytes {}, {delta})",
        c.input_format, c.input_size, c.outline, c.output_format, c.output_size
    );
    if let Some(family) = &c.family {
        for_llm.push_str(&format!(
            "\n@font-face {{ font-family: '{}'; src: url('{filename}') format('{}'); font-display: swap; }}",
            family,
            css_format_hint(c.ext)
        ));
    }

    let data_url = format!("data:{mime};base64,{}", B64.encode(&c.bytes));
    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime, filename },
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
                    "url":    { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["woff2", "woff", "ttf", "otf"], "description": "Target font container: woff2 (best-compressed web font, Brotli), woff (older web font, zlib), ttf or otf (uncompressed desktop font). The input format (TTF/OTF/WOFF/WOFF2) is auto-detected. The glyph outline technology is preserved — ttf↔otf changes the container/extension, it does not re-outline glyphs." }
                },
                "required": ["format"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

//! gizza-ai/font-info — inspect a TTF/OTF/WOFF/WOFF2 font and report its
//! name-table metadata, glyph count, vertical metrics, style classes,
//! embedding permissions (OS/2 `fsType`) and table directory.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::inspect` (pure: `wuff` normalises WOFF/WOFF2 back to SFNT, then
//! `ttf-parser` reads the tables) → flat JSON the LLM reads directly. Read-only
//! — the font is never modified; use `font-subset` or `woff2-convert` for that.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→JSON report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the no-page file-input
//! pattern, like elf-info / woff2-convert / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{AssetKind, Input, SkillError, SourceFields, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

// Desktop faces with large CJK glyph sets run to a few MB; 32 MiB fits every
// real font (and every web font by a wide margin).
const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a font arrives via URL fetch or
/// an attachment ref. No other parameters: the container (TTF/OTF/WOFF/WOFF2) is
/// auto-detected from the file's leading bytes.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FontInfoTool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/font-info",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect a TTF/OTF/WOFF/WOFF2 font: names, glyph count, metrics and embedding licence",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Inspect a font file (TTF, OTF, WOFF or WOFF2 — the container is auto-detected from the file's bytes, and web fonts are decompressed back to SFNT first) and report everything its tables declare. Returns the input container and glyph outline technology (TrueType glyf vs PostScript/CFF); the name-table strings (family, subfamily, typographic family/subfamily, full name, PostScript name, version, unique id, description, copyright, trademark, manufacturer, designer, vendor and designer URLs, licence description and licence URL, sample text); the glyph count; metrics in design units (units per em, ascender, descender, line gap, line height, OS/2 typographic ascender/descender/line gap, x-height, cap height, underline and strikeout position/thickness) and the global glyph bounding box; the style classification (OS/2 weight class 1-1000 with its CSS weight name, width class 1-9, slope Normal/Italic/Oblique, bold/italic/oblique/regular/monospaced/variable flags, italic angle); the OS/2 fsType embedding permission (Installable, Restricted, Preview & Print or Editable) in plain English with the raw bit field plus the subsetting and outline-embedding flags; the SFNT table directory (tag + uncompressed length) with flags for colour glyphs, hinting programs and OpenType layout; the cmap subtables with the number of distinct Unicode code points the font maps; and the fvar variation axes of a variable font. Useful for checking what a downloaded or licensed font actually permits, for auditing web-font payloads, and for verifying family/style metadata before shipping a font. Font collections (.ttc/.otc) and legacy formats (EOT, SVG, Type 1) are rejected with an explanatory error. This tool is read-only — use woff2-convert to change container or font-subset to shrink a font. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl FontInfoTool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::SkillResultExt;
    let args: Args = serde_json::from_slice(&body).invalid_args("font-info")?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let info = gizza_ai_font_info_core::inspect(&bytes).map_err(SkillError::InvalidArgs)?;

    serde_json::to_vec(&info)
        .map_err(|e| SkillError::Serialize(format!("serialize font-info response: {e}")))
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn manifest_tool_section_matches_the_live_descriptor() {
        // The page/CLI-facing manifest is generated from this block's descriptor
        // (scripts/sync-tool-manifest.py) — guard it against silent drift.
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            manifest["tool"]["parameters"], derived,
            "manifest.json tool.parameters drifted from descriptor()"
        );
        assert_eq!(manifest["name"], "gizza-ai/font-info");
    }

    #[test]
    fn response_serializes_the_core_report() {
        const TTF: &[u8] = include_bytes!("../core/tests/fixtures/sample.ttf");
        let info = gizza_ai_font_info_core::inspect(TTF).unwrap();
        let json: serde_json::Value = serde_json::to_value(&info).unwrap();
        assert_eq!(json["input_format"], "TTF");
        assert_eq!(json["outline"], "TrueType (glyf)");
        assert_eq!(json["names"]["family"], "Gizza Sample");
        assert_eq!(json["glyph_count"], 4);
        assert_eq!(json["metrics"]["units_per_em"], 1000);
        assert_eq!(json["embedding"]["permission"], "Preview & Print");
        assert!(json["tables"].as_array().unwrap().len() > 1);
    }
}

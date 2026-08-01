//! gizza-ai/gdrive-link-converter — turn any Google Drive link into the link you
//! actually need (direct download, inline embed, share, preview, thumbnail) or
//! just the file ID.
//!
//! Thin chat-skill wrapper around `gizza-ai-gdrive-link-converter-core`. The chat
//! schema is derived from `descriptor()` (single source — shared across chat +
//! CLI + page query-params); the handler delegates to `block_utils::run_skill`.
//! No host calls — runs entirely inside the WASM sandbox (pure string work).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_gdrive_link_converter_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_size")]
    size: String,
    #[serde(default)]
    per_line: bool,
}

fn default_output() -> String {
    "direct".to_string()
}

fn default_size() -> String {
    "w1000".to_string()
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input").required().describe(
                "A Google Drive link in any shape (drive.google.com/file/d/FILE_ID/view, open?id=FILE_ID, uc?export=download&id=FILE_ID, drive.usercontent.google.com/download?id=FILE_ID, docs.google.com/document/d/FILE_ID, a /folders/FILE_ID folder link) or a bare file ID. With per_line=true, one link per line.",
            ),
        )
        .param(
            Param::enumv(
                "output",
                ["direct", "direct_confirm", "view", "share", "preview", "thumbnail", "id"],
            )
            .default("direct")
            .describe(
                "Which link to produce. 'direct' = classic download link (uc?export=download, small files); 'direct_confirm' = large-file download that skips the virus-scan warning (drive.usercontent.google.com …&confirm=t); 'view' = inline image embed (uc?export=view); 'share' = the human share/view link (the 'back' conversion); 'preview' = iframe-embeddable preview URL; 'thumbnail' = resizable thumbnail image; 'id' = the bare file ID for scripts. Default 'direct'.",
            ),
        )
        .param(
            Param::string("size").default("w1000").describe(
                "Thumbnail size token when output='thumbnail' — Google Drive 'sz' syntax: w<pixels> (e.g. w500), w<W>-h<H> (e.g. w320-h240), or s<pixels>. Ignored for other outputs. Default 'w1000'.",
            ),
        )
        .param(
            Param::boolean("per_line").default(false).describe(
                "When true, convert each line of the input independently (rejoined with newlines, blank lines preserved) — for a batch of Drive links. Default false.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GdriveLinkConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gdrive-link-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Google Drive link to a direct-download, embed, preview, or thumbnail URL",
    skill(
        description = "Convert a Google Drive share/view link into the link you actually need: a direct-download link, a large-file download link that skips the virus-scan warning, an inline image-embed link, a preview iframe URL, a resizable thumbnail URL, the share/view link (the reverse conversion), or the bare file ID for scripts. Accepts every common Drive URL shape (file/d/ID/view, open?id=, uc?export=download&id=, drive.usercontent.google.com/download?id=, docs.google.com/…/d/ID, /folders/ID) plus a bare ID. Set output to direct|direct_confirm|view|share|preview|thumbnail|id (default direct); size sets the thumbnail sz token; per_line=true converts a batch of links (one per line). Pure string work — it never contacts Google.",
        parameters = schema_json()
    ),
)]
impl GdriveLinkConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gdrive-link-converter", |a: Args| {
            convert(&a.input, &a.output, &a.size, a.per_line).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "A Google Drive link in any shape (drive.google.com/file/d/FILE_ID/view, open?id=FILE_ID, uc?export=download&id=FILE_ID, drive.usercontent.google.com/download?id=FILE_ID, docs.google.com/document/d/FILE_ID, a /folders/FILE_ID folder link) or a bare file ID. With per_line=true, one link per line." },
                    "output": { "type": "string", "enum": ["direct", "direct_confirm", "view", "share", "preview", "thumbnail", "id"], "default": "direct", "description": "Which link to produce. 'direct' = classic download link (uc?export=download, small files); 'direct_confirm' = large-file download that skips the virus-scan warning (drive.usercontent.google.com …&confirm=t); 'view' = inline image embed (uc?export=view); 'share' = the human share/view link (the 'back' conversion); 'preview' = iframe-embeddable preview URL; 'thumbnail' = resizable thumbnail image; 'id' = the bare file ID for scripts. Default 'direct'." },
                    "size": { "type": "string", "default": "w1000", "description": "Thumbnail size token when output='thumbnail' — Google Drive 'sz' syntax: w<pixels> (e.g. w500), w<W>-h<H> (e.g. w320-h240), or s<pixels>. Ignored for other outputs. Default 'w1000'." },
                    "per_line": { "type": "boolean", "default": false, "description": "When true, convert each line of the input independently (rejoined with newlines, blank lines preserved) — for a batch of Drive links. Default false." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

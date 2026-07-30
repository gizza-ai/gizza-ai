//! gizza-ai/iso9660-toc-list — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    format: Option<String>,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The .iso disc image encoded as base64 (standard or URL-safe alphabet, \
                     whitespace tolerated). Only the volume descriptors and directory extents \
                     are read — file contents are never touched. The decoded bytes must carry \
                     the 'CD001' identifier at sector 16 (offset 0x8001). Example: run \
                     `base64 disc.iso` and paste the result.",
        ))
        .param(
            Param::enumv("format", ["tree", "list", "summary"])
                .default("tree")
                .describe(
                    "Output shape: tree (default) — an indented directory tree with per-file \
                     sizes and the volume label; list — one full path per line (directories end \
                     with '/', files show their size); summary — volume label, standard \
                     (ISO 9660 / Joliet), block size, image size and file/directory counts.",
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
    name = "gizza-ai/iso9660-toc-list",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List the directory tree, file sizes and volume label of an ISO 9660 disc image",
    skill(
        description = "List the contents of an ISO 9660 (CD/DVD) disc image supplied as base64, \
            without extracting any files. Returns the directory tree with per-file sizes and the \
            volume label. Reads only the volume descriptors and directory records — never the file \
            data — so it is fast and safe on large images. Prefers Joliet long filenames when the \
            image carries them, otherwise uses the standard ISO 9660 names (with the ';1' version \
            suffix stripped). Choose format=tree for an indented tree, list for flat paths, or \
            summary for label/standard/block-size/counts. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "iso9660-toc-list", |a: Args| {
            gizza_ai_iso9660_toc_list_core::run(&a.data, a.format.as_deref().unwrap_or("tree"))
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The .iso disc image encoded as base64 (standard or URL-safe alphabet, whitespace tolerated). Only the volume descriptors and directory extents are read — file contents are never touched. The decoded bytes must carry the 'CD001' identifier at sector 16 (offset 0x8001). Example: run `base64 disc.iso` and paste the result." },
                    "format": { "type": "string", "enum": ["tree", "list", "summary"], "default": "tree", "description": "Output shape: tree (default) — an indented directory tree with per-file sizes and the volume label; list — one full path per line (directories end with '/', files show their size); summary — volume label, standard (ISO 9660 / Joliet), block size, image size and file/directory counts." }
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

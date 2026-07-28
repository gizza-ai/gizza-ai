//! gizza-ai/simplenote-export-to-markdown — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Converts a Simplenote (or
//! Evernote-style) JSON export into a labeled bundle of clean Markdown files.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    filename_style: String,
    #[serde(default)]
    metadata: String,
    #[serde(default)]
    include_trashed: bool,
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The JSON from your Simplenote export (simplenote.json). Also accepts a legacy Simplenote export or an Evernote-style JSON array of note objects. Paste the raw JSON text."),
        )
        .param(
            Param::enumv("filename_style", ["date-title", "title", "id"])
                .default("date-title")
                .describe("How each note's filename is built. 'date-title' (default) prefixes the slugged title with the note's creation date (YYYY-MM-DD-title.md); 'title' uses just the slugged title (title.md); 'id' uses the note's id/key. Collisions get a numeric suffix."),
        )
        .param(
            Param::enumv("metadata", ["frontmatter", "inline"])
                .default("frontmatter")
                .describe("How tags and dates are surfaced. 'frontmatter' (default) writes a YAML block with title, created/updated dates, tags, and pinned/markdown flags; 'inline' writes no frontmatter and appends tags as #hashtags at the bottom."),
        )
        .param(
            Param::boolean("include_trashed")
                .default(false)
                .describe("Include trashed/deleted notes (Simplenote's trashedNotes, or notes flagged deleted). Default false — only active notes are exported."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/simplenote-export-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Simplenote JSON export into clean Markdown files.",
    skill(
        description = "Convert a Simplenote (or Evernote-style) JSON export into a labeled bundle of clean Markdown files — one per note, each with a title heading, tags, and dates. Auto-detects the modern Simplenote export ({activeNotes, trashedNotes}), a legacy Simplenote export, or an Evernote-style JSON array. filename_style controls the filename (date-title default, title, or id); metadata='frontmatter' (default) writes YAML frontmatter with title/created/updated/tags/pinned, while metadata='inline' appends tags as #hashtags; include_trashed=true also exports trashed notes. Output is one bundle: each file is preceded by an '==== filename ====' header.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "simplenote-export-to-markdown", |a: Args| {
            gizza_ai_simplenote_export_to_markdown_core::convert(
                &a.input,
                &a.filename_style,
                &a.metadata,
                a.include_trashed,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The JSON from your Simplenote export (simplenote.json). Also accepts a legacy Simplenote export or an Evernote-style JSON array of note objects. Paste the raw JSON text." },
                    "filename_style": { "type": "string", "enum": ["date-title", "title", "id"], "default": "date-title", "description": "How each note's filename is built. 'date-title' (default) prefixes the slugged title with the note's creation date (YYYY-MM-DD-title.md); 'title' uses just the slugged title (title.md); 'id' uses the note's id/key. Collisions get a numeric suffix." },
                    "metadata": { "type": "string", "enum": ["frontmatter", "inline"], "default": "frontmatter", "description": "How tags and dates are surfaced. 'frontmatter' (default) writes a YAML block with title, created/updated dates, tags, and pinned/markdown flags; 'inline' writes no frontmatter and appends tags as #hashtags at the bottom." },
                    "include_trashed": { "type": "boolean", "default": false, "description": "Include trashed/deleted notes (Simplenote's trashedNotes, or notes flagged deleted). Default false — only active notes are exported." }
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

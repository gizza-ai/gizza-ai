//! gizza-ai/keep-to-markdown — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Converts a Google Takeout
//! Keep export (JSON or HTML) into Markdown notes with labels and checkboxes.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    metadata: String,
    #[serde(default)]
    filename_style: String,
    #[serde(default)]
    checkbox_style: String,
    #[serde(default = "yes")]
    include_archived: bool,
    #[serde(default)]
    include_trashed: bool,
    #[serde(default = "yes")]
    link_attachments: bool,
}

fn yes() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Your Google Keep Takeout export, pasted as text: one note's `.json` file from `Takeout/Keep/`, a JSON array of several such notes, or the Keep `.html` export of a note. The format is auto-detected."),
        )
        .param(
            Param::enumv("metadata", ["frontmatter", "inline", "none"])
                .default("frontmatter")
                .describe("Where each note's metadata goes. 'frontmatter' (default) writes a YAML block with title, created/updated timestamps, labels, and the pinned/archived/color flags; 'inline' writes no YAML and appends the labels as #hashtags at the bottom; 'none' emits just the heading and the body."),
        )
        .param(
            Param::enumv("filename_style", ["date-title", "title", "label-title"])
                .default("date-title")
                .describe("How each note's filename is built. 'date-title' (default) prefixes the slugged title with the creation date (2026-01-15-grocery-list.md); 'title' uses just the slugged title; 'label-title' puts the note in a folder named after its first label (shopping/grocery-list.md, or unlabeled/ when it has none). Duplicate names get a -1, -2 suffix."),
        )
        .param(
            Param::enumv("checkbox_style", ["task-list", "bullet", "plain"])
                .default("task-list")
                .describe("How Keep checklist items render. 'task-list' (default) writes Markdown task list items ('- [ ] Milk' / '- [x] Eggs'); 'bullet' writes plain '- Milk' bullets and drops the checked state; 'plain' writes one bare line per item."),
        )
        .param(
            Param::boolean("include_archived")
                .default(true)
                .describe("Include archived notes (Keep's isArchived / the Archived chip in the HTML export). Default true; they are tagged 'archived: true' in the frontmatter. Set false to export only unarchived notes."),
        )
        .param(
            Param::boolean("include_trashed")
                .default(false)
                .describe("Include trashed notes (Keep's isTrashed). Default false — deleted notes are skipped."),
        )
        .param(
            Param::boolean("link_attachments")
                .default(true)
                .describe("Emit a Markdown link for each attachment listed in the note — '![photo.jpg](photo.jpg)' for images, '[voice.3gp](voice.3gp)' otherwise. Default true. The attachment files themselves live beside the notes in your Takeout folder; only the links are written here."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/keep-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Google Keep Takeout export into Markdown notes",
    skill(
        description = "Convert a Google Takeout Keep export into Markdown notes with labels and checkboxes preserved. Paste one note's `.json` from `Takeout/Keep/`, a JSON array of such notes, or the Keep `.html` export — the format is auto-detected. Each note becomes one Markdown file in a single bundle, preceded by an '==== filename.md ====' header: the title becomes an `#` heading, `listContent` items become '- [ ]' / '- [x]' task list items (checkbox_style also offers bullet/plain), and labels, timestamps (createdTimestampUsec/userEditedTimestampUsec, converted to ISO-8601 UTC), pinned/archived flags and the note color go into YAML frontmatter (metadata='inline' turns labels into #hashtags instead, 'none' drops them). filename_style picks date-title (default), title, or label-title (first label as a folder). include_archived (default true), include_trashed (default false) and link_attachments (default true) control what is exported. Runs locally — nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "keep-to-markdown", |a: Args| {
            gizza_ai_keep_to_markdown_core::convert(
                &a.input,
                &a.metadata,
                &a.filename_style,
                &a.checkbox_style,
                a.include_archived,
                a.include_trashed,
                a.link_attachments,
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
                    "input": { "type": "string", "description": "Your Google Keep Takeout export, pasted as text: one note's `.json` file from `Takeout/Keep/`, a JSON array of several such notes, or the Keep `.html` export of a note. The format is auto-detected." },
                    "metadata": { "type": "string", "enum": ["frontmatter", "inline", "none"], "default": "frontmatter", "description": "Where each note's metadata goes. 'frontmatter' (default) writes a YAML block with title, created/updated timestamps, labels, and the pinned/archived/color flags; 'inline' writes no YAML and appends the labels as #hashtags at the bottom; 'none' emits just the heading and the body." },
                    "filename_style": { "type": "string", "enum": ["date-title", "title", "label-title"], "default": "date-title", "description": "How each note's filename is built. 'date-title' (default) prefixes the slugged title with the creation date (2026-01-15-grocery-list.md); 'title' uses just the slugged title; 'label-title' puts the note in a folder named after its first label (shopping/grocery-list.md, or unlabeled/ when it has none). Duplicate names get a -1, -2 suffix." },
                    "checkbox_style": { "type": "string", "enum": ["task-list", "bullet", "plain"], "default": "task-list", "description": "How Keep checklist items render. 'task-list' (default) writes Markdown task list items ('- [ ] Milk' / '- [x] Eggs'); 'bullet' writes plain '- Milk' bullets and drops the checked state; 'plain' writes one bare line per item." },
                    "include_archived": { "type": "boolean", "default": true, "description": "Include archived notes (Keep's isArchived / the Archived chip in the HTML export). Default true; they are tagged 'archived: true' in the frontmatter. Set false to export only unarchived notes." },
                    "include_trashed": { "type": "boolean", "default": false, "description": "Include trashed notes (Keep's isTrashed). Default false — deleted notes are skipped." },
                    "link_attachments": { "type": "boolean", "default": true, "description": "Emit a Markdown link for each attachment listed in the note — '![photo.jpg](photo.jpg)' for images, '[voice.3gp](voice.3gp)' otherwise. Default true. The attachment files themselves live beside the notes in your Takeout folder; only the links are written here." }
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

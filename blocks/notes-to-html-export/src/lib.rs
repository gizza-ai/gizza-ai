//! gizza-ai/notes-to-html-export — combine pasted Markdown notes into one
//! self-contained HTML document with embedded CSS and a linked table of contents.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI and the generated page manifest); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    notes: String,
    #[serde(default)]
    split: Option<String>,
    #[serde(default)]
    toc: Option<String>,
    #[serde(default)]
    toc_depth: Option<u32>,
    #[serde(default)]
    number_sections: Option<bool>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    theme: Option<String>,
}

/// Single source for the chat schema (and CLI). Mirrors the web page controls.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("notes").required().describe("Markdown notes to bundle. Paste one or more notes; split can treat level-1 headings or thematic breaks as note boundaries."))
        .param(Param::enumv("split", ["heading", "hr"]).default("heading").describe("How to split the pasted body into notes: heading starts a new note at each level-1 # heading; hr splits on a thematic break line such as --- or ***. Default heading."))
        .param(Param::enumv("toc", ["sidebar", "top", "none"]).default("sidebar").describe("Where to place the linked table of contents: sticky sidebar, inline top block, or none. Default sidebar."))
        .param(Param::integer("toc_depth").default(3).min(1.0).max(6.0).describe("Deepest heading level included in the table of contents, 1-6. Default 3."))
        .param(Param::boolean("number_sections").default(false).describe("Prefix headings and TOC entries with section numbers such as 1, 1.1, 1.1.1. Default false."))
        .param(Param::string("title").default("Notes").describe("Document title used in the HTML <title> and visible page heading. Default Notes."))
        .param(Param::enumv("theme", ["light", "dark", "auto"]).default("light").describe("Embedded reading theme: light, dark, or auto (follows the reader's OS color scheme). Default light."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/notes-to-html-export",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bundle Markdown notes into a self-contained HTML page",
    skill(
        description = "Bundle a pasted set of Markdown notes into one standalone HTML document with embedded CSS, sanitized rendered Markdown, a linked table of contents, optional section numbering, note splitting by level-1 headings or thematic breaks, and light/dark/auto reading themes. The output is a complete <!doctype html> file string with no external assets or JavaScript.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "notes-to-html-export", |a: Args| {
            gizza_ai_notes_to_html_export_core::export_notes(
                &a.notes,
                a.split.as_deref().unwrap_or("heading"),
                a.toc.as_deref().unwrap_or("sidebar"),
                a.toc_depth.unwrap_or(3),
                a.number_sections.unwrap_or(false),
                a.title.as_deref().unwrap_or("Notes"),
                a.theme.as_deref().unwrap_or("light"),
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(props["notes"]["type"], "string");
        assert_eq!(props["split"]["enum"], serde_json::json!(["heading", "hr"]));
        assert_eq!(
            props["toc"]["enum"],
            serde_json::json!(["sidebar", "top", "none"])
        );
        assert_eq!(props["toc_depth"]["minimum"], 1);
        assert_eq!(props["toc_depth"]["maximum"], 6);
        assert_eq!(props["number_sections"]["default"], false);
        assert_eq!(props["title"]["default"], "Notes");
        assert_eq!(
            props["theme"]["enum"],
            serde_json::json!(["light", "dark", "auto"])
        );
        assert_eq!(schema["required"], serde_json::json!(["notes"]));
    }
}

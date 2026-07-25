//! gizza-ai/pdf-metadata-edit — view or edit a PDF's document metadata.
//!
//! Loads the source PDF (URL/ref) and either reports its current `/Info`
//! dictionary fields (`mode = view`) or sets/clears Title, Author, Subject, and
//! Keywords and returns the new PDF as a base64 envelope (`mode = edit`, the
//! default). For edit, an omitted or empty field is left untouched, so a caller
//! can change one field without wiping the others. `Input::Document` + `mode` +
//! optional `title` / `author` / `subject` / `keywords`. Chat + CLI only (no
//! page surface — a standalone page can't fetch an arbitrary PDF).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{resolve_source, respond_ok, AssetKind};
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

/// PDF byte cap (16 MiB) — generous for documents while bounding the memory a
/// single edit pulls into the wasm sandbox.
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    keywords: Option<String>,
}
fn default_mode() -> String {
    "edit".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("mode", ["edit", "view"])
                .default("edit")
                .describe("What to do: 'edit' (default) sets the metadata fields you provide; 'view' reads and returns the PDF's current metadata without changing it."),
        )
        .param(
            Param::string("title")
                .describe("New document title (edit mode). Leave empty or omit to keep the existing title unchanged."),
        )
        .param(
            Param::string("author")
                .describe("New document author (edit mode). Leave empty or omit to keep the existing author unchanged."),
        )
        .param(
            Param::string("subject")
                .describe("New document subject (edit mode). Leave empty or omit to keep the existing subject unchanged."),
        )
        .param(
            Param::string("keywords")
                .describe("New document keywords, as a comma-separated list (edit mode). Leave empty or omit to keep the existing keywords unchanged."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfMetadataEdit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-metadata-edit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "View and edit a PDF's title, author, subject, and keywords metadata",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "View or edit a PDF's document metadata (the Title, Author, Subject, and Keywords stored in its Info dictionary). With `mode` = 'view', it reads and returns the PDF's current metadata (including read-only Creator/Producer) without changing anything. With `mode` = 'edit' (the default), it sets the fields you provide and returns the updated PDF: pass `title`, `author`, `subject`, and/or `keywords` (keywords is a comma-separated list). An omitted or empty field is left unchanged, so you can update one field without wiping the others; provide at least one field to edit. Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfMetadataEdit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Map an optional chat string to a [`FieldUpdate`]: omitted or empty → leave
/// the existing value; a non-empty value → set it. (Clearing a field is
/// supported by the core but intentionally not reachable from chat, matching the
/// "empty means leave" convention.)
#[cfg(target_arch = "wasm32")]
fn field_update(v: Option<String>) -> gizza_ai_pdf_metadata_edit_core::FieldUpdate {
    use gizza_ai_pdf_metadata_edit_core::FieldUpdate;
    match v {
        Some(s) if !s.trim().is_empty() => FieldUpdate::Set(s),
        _ => FieldUpdate::Leave,
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::SkillResultExt;
    use gizza_ai_pdf_metadata_edit_core::{edit, read_info, Updates};

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-metadata-edit")?;
    let mode = args.mode.trim().to_ascii_lowercase();
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    match mode.as_str() {
        "view" | "read" => {
            let info = read_info(&bytes).map_err(SkillError::InvalidArgs)?;
            respond_ok(&info_json(&filename, &info))
        }
        "edit" | "set" | "write" | "" => {
            let updates = Updates {
                title: field_update(args.title),
                author: field_update(args.author),
                subject: field_update(args.subject),
                keywords: field_update(args.keywords),
            };
            if updates.is_noop() {
                return Err(SkillError::InvalidArgs(
                    "provide at least one of title, author, subject, or keywords to edit".into(),
                ));
            }
            let result = edit(&bytes, &updates).map_err(SkillError::InvalidArgs)?;
            edit_envelope(&filename, result)
        }
        other => Err(SkillError::InvalidArgs(format!(
            "invalid mode '{other}' (expected 'view' or 'edit')"
        ))),
    }
}

/// Shape the view-mode metadata into a plain JSON object for the LLM.
#[cfg(target_arch = "wasm32")]
fn info_json(filename: &str, info: &gizza_ai_pdf_metadata_edit_core::Info) -> serde_json::Value {
    serde_json::json!({
        "filename": filename,
        "title": info.title,
        "author": info.author,
        "subject": info.subject,
        "keywords": info.keywords,
        "creator": info.creator,
        "producer": info.producer,
    })
}

/// Build the edit-mode PDF envelope (base64 data URL + LLM summary).
#[cfg(target_arch = "wasm32")]
fn edit_envelope(
    filename: &str,
    result: gizza_ai_pdf_metadata_edit_core::EditResult,
) -> Result<Vec<u8>, SkillError> {
    let out_len = result.bytes.len();
    let encoded = B64.encode(&result.bytes);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|s| format!("{s}-metadata.pdf"))
        .unwrap_or_else(|| "metadata.pdf".to_string());

    let for_llm = format!(
        "updated {} of {filename} ({out_len}-byte PDF)",
        result.changed.join(", ")
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename: out_name,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["edit", "view"], "default": "edit", "description": "What to do: 'edit' (default) sets the metadata fields you provide; 'view' reads and returns the PDF's current metadata without changing it." },
                    "title":    { "type": "string", "description": "New document title (edit mode). Leave empty or omit to keep the existing title unchanged." },
                    "author":   { "type": "string", "description": "New document author (edit mode). Leave empty or omit to keep the existing author unchanged." },
                    "subject":  { "type": "string", "description": "New document subject (edit mode). Leave empty or omit to keep the existing subject unchanged." },
                    "keywords": { "type": "string", "description": "New document keywords, as a comma-separated list (edit mode). Leave empty or omit to keep the existing keywords unchanged." }
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
    fn args_default_mode_is_edit() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/doc.pdf"}"#).unwrap();
        assert_eq!(a.mode, "edit");
        assert!(a.title.is_none());
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}

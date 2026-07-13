//! gizza-ai/sqlite-db-inspector — inspect an uploaded SQLite database schema.
//!
//! No-page block (chat + CLI surface only): it ingests a binary `.db`/`.sqlite`
//! file, so the browser-local text form scaffold is not a good fit. The tool
//! reads `sqlite_master` and rowid table b-trees directly through the proven
//! sqlite-table-to-csv parser; it does not run user-supplied SQL.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_sqlite_db_inspector_core::{inspect_database, render_report, Options, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_LLM_CHARS: usize = 16 * 1024;

#[derive(Debug, Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    include_internal: Option<bool>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("format", ["markdown", "json"])
                .default("markdown")
                .describe("Output format: markdown for a readable schema report (default) or json for structured table/index/view metadata."),
        )
        .param(
            Param::boolean("include_internal")
                .default(false)
                .describe("Include SQLite internal sqlite_* objects such as autoindexes. Default false; user tables still mention their auto-created indexes when relevant."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from_args(args: &Args) -> Result<Options, SkillError> {
    let format = match args.format.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => OutputFormat::parse(s).map_err(SkillError::InvalidArgs)?,
        None => OutputFormat::Markdown,
    };
    Ok(Options {
        format,
        include_internal: args.include_internal.unwrap_or(false),
    })
}

fn summarize(rendered: &str) -> String {
    if rendered.chars().count() <= MAX_LLM_CHARS {
        rendered.to_string()
    } else {
        let head: String = rendered.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "(first {MAX_LLM_CHARS} of {} chars; full report is available in the download)\n{head}",
            rendered.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct SqliteDbInspector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sqlite-db-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect SQLite tables, columns, indexes, foreign keys, views, and row counts",
    requires = ["wafer-run/network"],
    skill(
        description = "Inspect an uploaded SQLite .db/.sqlite file without running user SQL. Provide the database by public http/https url or uploaded attachment ref. The tool reads the on-disk schema catalog to list tables, columns, explicit and auto-created indexes, foreign keys, views, triggers, and row counts for normal rowid tables; WITHOUT ROWID row counts are reported as unavailable rather than guessed. Choose markdown or json output, and optionally include sqlite_* internal objects.",
        parameters = schema_json()
    ),
)]
impl SqliteDbInspector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("sqlite-db-inspector")?;
    let opts = options_from_args(&args)?;
    let (bytes, _mime, _filename) = resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;
    let report = inspect_database(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let rendered = render_report(&report, opts.format);
    let (mime, filename) = match opts.format {
        OutputFormat::Json => ("application/json", "sqlite-inspection.json"),
        OutputFormat::Markdown => ("text/markdown", "sqlite-inspection.md"),
    };
    let env = Envelope {
        for_llm: summarize(&rendered),
        for_ui: ForUi {
            data_url: format!("data:{mime};base64,{}", B64.encode(rendered.as_bytes())),
            mime: mime.to_string(),
            filename: filename.to_string(),
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
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": {
                        "type": "string",
                        "enum": ["markdown", "json"],
                        "default": "markdown",
                        "description": "Output format: markdown for a readable schema report (default) or json for structured table/index/view metadata."
                    },
                    "include_internal": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include SQLite internal sqlite_* objects such as autoindexes. Default false; user tables still mention their auto-created indexes when relevant."
                    }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn options_defaults_and_parse() {
        let args: Args = serde_json::from_str(r#"{"url":"https://example.com/db.sqlite"}"#).unwrap();
        let opts = options_from_args(&args).unwrap();
        assert_eq!(opts.format, OutputFormat::Markdown);
        assert!(!opts.include_internal);

        let args: Args = serde_json::from_str(
            r#"{"ref":"call_1","format":"json","include_internal":true}"#,
        )
        .unwrap();
        let opts = options_from_args(&args).unwrap();
        assert_eq!(opts.format, OutputFormat::Json);
        assert!(opts.include_internal);
    }

    #[test]
    fn options_reject_bad_format() {
        let args: Args = serde_json::from_str(r#"{"url":"u","format":"xml"}"#).unwrap();
        assert!(options_from_args(&args).is_err());
    }

    #[test]
    fn args_require_exactly_one_source() {
        assert!(serde_json::from_str::<Args>(r#"{"format":"json"}"#).is_err());
        assert!(serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).is_err());
    }
}

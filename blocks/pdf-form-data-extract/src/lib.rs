//! gizza-ai/pdf-form-data-extract — read the filled values of an AcroForm
//! (interactive) PDF and return each field's name / type / value as JSON or CSV.
//!
//! No-page block (chat + CLI surface only, like `blocks/pdf-table-extract` and
//! `blocks/pdf-extract-text`): a PDF is a binary file input and the output is
//! delimited text / JSON, which fits neither the pure-text page shape nor the
//! ffmpeg file→media page shape — so there is no standalone page.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
//!
//! Pipeline: parse `{url|ref}` + `format`/`delimiter`/`include_empty` → resolve
//! bytes via `block_utils::resolve_source` (URL fetch or attachment lookup,
//! validated to the `application/*` `AssetKind::Document` class) →
//! `core::extract_form_data` (lopdf AcroForm walk) → optionally drop unfilled
//! fields → serialize with `core::to_csv` / `core::to_json` → emit a text
//! `Envelope`. The LLM sees the field table (head-truncated if large); the UI
//! gets a downloadable `data:` URL + `*.csv`/`*.tsv`/`*.json` filename.

// The #[wafer_block] macro emits wasm-only registration; supporting imports and
// the Args type are only used inside that impl. `descriptor()` / `schema_json()`
// remain native-compilable so the drift-guard + unit tests below can run.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
// The source resolver calls the wasm-gated network/attachment host imports; the
// descriptor and arg parsing are host-testable.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_form_data_extract_core::{extract_form_data, to_csv, to_json, FormField};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the PDF input we accept (matches pdf-table-extract; lopdf holds the
/// whole document in memory).
const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Cap on the field text fed back to the LLM (`for_llm`). Larger results are
/// head-truncated with a note; the full output is always in the download.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// Output format: `json` (default) or `csv`.
    #[serde(default)]
    format: Option<String>,
    /// CSV field delimiter: `comma` (default), `semicolon`, or `tab`.
    #[serde(default)]
    delimiter: Option<String>,
    /// Include fields that have no value (default true = full field map).
    #[serde(default)]
    include_empty: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf`. The drift-guard test below proves the derived
/// schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("format", ["json", "csv"]).default("json").describe(
                "Output format: \"json\" (array of {name, type, value} objects) or \"csv\" (delimited name,type,value rows). Default \"json\".",
            ),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab"]).default("comma").describe(
                "Field delimiter for csv output: \"comma\" (.csv), \"semicolon\", or \"tab\" (.tsv). Ignored when format is json. Default \"comma\".",
            ),
        )
        .param(
            Param::boolean("include_empty").default(true).describe(
                "Include fields that have no value (unfilled text fields, unchecked boxes). Set false to return only filled fields. Default true.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Resolve `format` to `(serialized_text, mime, extension)`; errors on an
/// unknown format/delimiter value.
fn serialize(
    fields: &[FormField],
    format: &str,
    delimiter: &str,
) -> Result<(String, &'static str, &'static str), SkillError> {
    match format {
        "json" => Ok((to_json(fields), "application/json", "json")),
        "csv" => {
            let (delim, ext, mime) = match delimiter {
                "comma" => (',', "csv", "text/csv"),
                "semicolon" => (';', "csv", "text/csv"),
                "tab" => ('\t', "tsv", "text/tab-separated-values"),
                other => {
                    return Err(SkillError::InvalidArgs(format!(
                        "unknown delimiter {other:?}; use \"comma\", \"semicolon\", or \"tab\""
                    )))
                }
            };
            Ok((to_csv(fields, delim), mime, ext))
        }
        other => Err(SkillError::InvalidArgs(format!(
            "unknown format {other:?}; use \"json\" or \"csv\""
        ))),
    }
}

/// Head-truncate `text` to `MAX_LLM_CHARS` for the LLM view, with a note.
fn clip_for_llm(text: &str) -> String {
    if text.chars().count() <= MAX_LLM_CHARS {
        text.to_string()
    } else {
        let head: String = text.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "{head}\n… (truncated to {MAX_LLM_CHARS} of {} chars; full output in the download)",
            text.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct PdfFormDataExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-form-data-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract AcroForm field values from a PDF as JSON or CSV",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Read the filled values of an interactive (AcroForm) PDF and return each field's name, type, and value as JSON or CSV. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Field names are fully qualified (nested subform fields are dotted, e.g. `form1.first`) and types are classified as text, checkbox, radio, pushbutton, dropdown, listbox, or signature. Choose `format` (json/csv), a csv `delimiter` (comma/semicolon/tab), and whether to `include_empty` unfilled fields. Reads the AcroForm layer (incl. UTF-16BE field names on IRS-style forms); it does not parse pure XFA/XML-only forms, and a PDF with no form fields is reported as an error.",
        parameters = schema_json()
    ),
)]
impl PdfFormDataExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-form-data-extract")?;

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let all = extract_form_data(&bytes).map_err(SkillError::InvalidArgs)?;

    let include_empty = args.include_empty.unwrap_or(true);
    let fields: Vec<FormField> = if include_empty {
        all
    } else {
        all.into_iter().filter(|f| f.filled).collect()
    };

    let format = args.format.as_deref().unwrap_or("json");
    let delimiter = args.delimiter.as_deref().unwrap_or("comma");
    let (text, mime, ext) = serialize(&fields, format, delimiter)?;

    let nfields = fields.len();
    let summary = if nfields == 0 {
        if include_empty {
            format!("The AcroForm in {filename} has no fields.")
        } else {
            format!("No filled fields in {filename} (every field is empty/unchecked).")
        }
    } else {
        format!("Read {nfields} form field(s) from {filename}:")
    };
    let for_llm = if text.trim().is_empty() {
        summary
    } else {
        format!("{summary}\n{}", clip_for_llm(&text))
    };

    let out_filename = replace_extension(&filename, ext);
    let data_url = format!("data:{mime};base64,{}", B64.encode(text.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: mime.to_string(),
            filename: out_filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety / no LLM-facing chat-schema drift: the descriptor-derived
    /// schema must match this authored one. `Input::Document` supplies the
    /// centralized `url`/`ref` wording + the `oneOf`; the tool params carry their
    /// `.describe()` text, enum variants, and defaults.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["json", "csv"], "default": "json", "description": "Output format: \"json\" (array of {name, type, value} objects) or \"csv\" (delimited name,type,value rows). Default \"json\"." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab"], "default": "comma", "description": "Field delimiter for csv output: \"comma\" (.csv), \"semicolon\", or \"tab\" (.tsv). Ignored when format is json. Default \"comma\"." },
                    "include_empty": { "type": "boolean", "default": true, "description": "Include fields that have no value (unfilled text fields, unchecked boxes). Set false to return only filled fields. Default true." }
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
    fn args_parse_url_with_options() {
        use gizza_ai_block_utils::Source;
        let a: Args = serde_json::from_str(
            r#"{"url":"https://x/y.pdf","format":"csv","delimiter":"tab","include_empty":false}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(ref u) if u == "https://x/y.pdf"));
        assert_eq!(a.format.as_deref(), Some("csv"));
        assert_eq!(a.delimiter.as_deref(), Some("tab"));
        assert_eq!(a.include_empty, Some(false));
    }

    #[test]
    fn args_parse_ref_defaults() {
        use gizza_ai_block_utils::Source;
        let a: Args = serde_json::from_str(r#"{"ref":"call_9"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(ref r) if r == "call_9"));
        assert!(a.format.is_none());
        assert!(a.include_empty.is_none());
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    fn sample() -> Vec<FormField> {
        vec![
            FormField {
                name: "full_name".into(),
                field_type: "text".into(),
                value: "Ada Lovelace".into(),
                filled: true,
            },
            FormField {
                name: "subscribe".into(),
                field_type: "checkbox".into(),
                value: "Off".into(),
                filled: false,
            },
        ]
    }

    #[test]
    fn serialize_csv_tsv_json_pick_mime_and_ext() {
        let fields = sample();
        let (csv, mime, ext) = serialize(&fields, "csv", "comma").unwrap();
        assert_eq!((mime, ext), ("text/csv", "csv"));
        assert_eq!(csv, "name,type,value\r\nfull_name,text,Ada Lovelace\r\nsubscribe,checkbox,Off\r\n");

        let (_tsv, mime, ext) = serialize(&fields, "csv", "tab").unwrap();
        assert_eq!((mime, ext), ("text/tab-separated-values", "tsv"));

        let (json, mime, ext) = serialize(&fields, "json", "comma").unwrap();
        assert_eq!((mime, ext), ("application/json", "json"));
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["name"], "full_name");
        assert_eq!(v[0]["value"], "Ada Lovelace");
    }

    #[test]
    fn serialize_rejects_unknown_format() {
        let err = serialize(&[], "xml", "comma").unwrap_err();
        assert!(err.to_string().contains("unknown format"));
    }

    #[test]
    fn serialize_rejects_unknown_delimiter() {
        let err = serialize(&sample(), "csv", "pipe").unwrap_err();
        assert!(err.to_string().contains("unknown delimiter"));
    }
}

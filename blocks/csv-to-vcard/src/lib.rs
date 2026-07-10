//! gizza-ai/csv-to-vcard — turns a CSV or JSON contact list into a single `.vcf`
//! file of vCards. Thin chat-skill wrapper around `gizza-ai-csv-to-vcard-core`.
//! The chat schema is single-sourced from `descriptor()` (shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    version: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The contact list. Either CSV with a header row (the first row names the columns, e.g. 'First Name,Last Name,Email,Mobile Phone,Company') or a JSON array of contact objects. Common column names are auto-mapped to vCard properties: name parts → N/FN, Email/Work Email/Home Email → EMAIL, Phone/Mobile/Home Phone/Fax → TEL (with HOME/WORK/CELL/FAX types), Company+Department → ORG, Job Title → TITLE, Street/City/State/Zip/Country → ADR, Website → URL, Birthday → BDAY, Notes → NOTE, Gender → GENDER (4.0)."),
        )
        .param(
            Param::enumv("input_format", ["auto", "csv", "json"])
                .default("auto")
                .describe("How to read the input. 'auto' (default) treats input starting with '[' or '{' as JSON, otherwise CSV; 'csv' or 'json' force one."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"])
                .default("auto")
                .describe("CSV column separator (ignored for JSON). 'auto' (default) sniffs the header row; otherwise force comma, semicolon, tab, or pipe."),
        )
        .param(
            Param::enumv("version", ["3.0", "4.0"])
                .default("3.0")
                .describe("vCard version to emit. '3.0' (default, RFC 2426) has the widest import support (iPhone, Android, Google, Outlook); '4.0' (RFC 6350) adds the GENDER property and uses lowercase TYPE values."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvToVcard;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-vcard",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a CSV or JSON contact list into a .vcf file of vCards.",
    skill(
        description = "Convert a CSV (with a header row) or JSON array of contacts into a single .vcf file of vCards for bulk import into iPhone, Android, Google, or Outlook contacts. Common column names are auto-mapped: First/Last/Full Name → N/FN, Email/Work Email → EMAIL, Phone/Mobile/Home Phone/Fax → TEL with HOME/WORK/CELL/FAX types, Company+Department → ORG, Job Title → TITLE, Street/City/State/Zip/Country → ADR, Website → URL, Birthday → BDAY, Notes → NOTE, Gender → GENDER (4.0 only). Set input_format=auto|csv|json, delimiter=auto|comma|semicolon|tab|pipe, and version=3.0|4.0. Values are RFC-escaped and long lines are folded at 75 octets.",
        parameters = schema_json()
    ),
)]
impl CsvToVcard {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-to-vcard", |a: Args| {
            gizza_ai_csv_to_vcard_core::to_vcards(
                &a.data,
                &a.input_format,
                &a.delimiter,
                &a.version,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The contact list. Either CSV with a header row (the first row names the columns, e.g. 'First Name,Last Name,Email,Mobile Phone,Company') or a JSON array of contact objects. Common column names are auto-mapped to vCard properties: name parts → N/FN, Email/Work Email/Home Email → EMAIL, Phone/Mobile/Home Phone/Fax → TEL (with HOME/WORK/CELL/FAX types), Company+Department → ORG, Job Title → TITLE, Street/City/State/Zip/Country → ADR, Website → URL, Birthday → BDAY, Notes → NOTE, Gender → GENDER (4.0)." },
                    "input_format": { "type": "string", "enum": ["auto", "csv", "json"], "default": "auto", "description": "How to read the input. 'auto' (default) treats input starting with '[' or '{' as JSON, otherwise CSV; 'csv' or 'json' force one." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "semicolon", "tab", "pipe"], "default": "auto", "description": "CSV column separator (ignored for JSON). 'auto' (default) sniffs the header row; otherwise force comma, semicolon, tab, or pipe." },
                    "version": { "type": "string", "enum": ["3.0", "4.0"], "default": "3.0", "description": "vCard version to emit. '3.0' (default, RFC 2426) has the widest import support (iPhone, Android, Google, Outlook); '4.0' (RFC 6350) adds the GENDER property and uses lowercase TYPE values." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

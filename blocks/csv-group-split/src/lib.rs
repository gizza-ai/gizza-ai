//! gizza-ai/csv-group-split — split one CSV into per-group files, zipped.
//! Pure-Rust (csv + zip), so it runs on all backends incl. the chat SW. The core
//! produces per-group CSV strings; this wrapper zips them into one downloadable
//! archive. Surfaces: chat + CLI (no page — a zip/binary output has no page mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::io::{Cursor, Write};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_group_split_core::split;
use serde::Deserialize;
use wafer_sdk::*;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

#[derive(Deserialize)]
struct Args {
    data: String,
    key: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to split."))
        .param(Param::string("key").required().describe("The key column to split on — a header name (header=true) or a 1-based index. One output file per distinct value."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header (kept in each output file, and matchable by name). Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

fn build_zip(parts: &[(String, String)]) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, content) in parts {
            zw.start_file(name, opts).map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(content.as_bytes()).map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }
    Ok(buf)
}

#[cfg(target_arch = "wasm32")]
struct CsvGroupSplit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-group-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a CSV into per-group files (zipped)",
    skill(
        description = "Split one CSV into multiple files, one per distinct value in a key column, and return them bundled as a ZIP. `key` is a header name (header=true) or a 1-based index; each output CSV keeps the header. Useful for breaking a big CSV into per-category files.",
        parameters = schema_json()
    )
)]
impl CsvGroupSplit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid csv-group-split args: {e}")))?;
    let delim = if args.delimiter.is_empty() { ",".to_string() } else { args.delimiter };
    let parts = split(&args.data, &args.key, args.header, &delim).map_err(SkillError::InvalidArgs)?;
    let zip = build_zip(&parts).map_err(SkillError::InvalidArgs)?;
    let out_len = zip.len();
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");
    let env = Envelope {
        for_llm: format!("split CSV into {} files by '{}' ({out_len}-byte ZIP)", parts.len(), args.key),
        for_ui: ForUi { data_url, mime: "application/zip".to_string(), filename: "split.zip".to_string() },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_roundtrips_the_parts() {
        let parts = vec![("A.csv".to_string(), "x\n1\n".to_string()), ("B.csv".to_string(), "x\n2\n".to_string())];
        let z = build_zip(&parts).unwrap();
        assert_eq!(&z[..2], b"PK");
        let mut ar = zip::ZipArchive::new(Cursor::new(z)).unwrap();
        assert_eq!(ar.len(), 2);
        use std::io::Read;
        let mut s = String::new();
        ar.by_name("A.csv").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "x\n1\n");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":      { "type": "string", "description": "The CSV text to split." },
                    "key":       { "type": "string", "description": "The key column to split on — a header name (header=true) or a 1-based index. One output file per distinct value." },
                    "header":    { "type": "boolean", "default": true, "description": "Treat the first row as a header (kept in each output file, and matchable by name). Default true." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

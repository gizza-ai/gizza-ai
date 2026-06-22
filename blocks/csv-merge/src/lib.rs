//! gizza-ai/csv-merge — concatenate multiple CSVs into one (header-union).
//! Loads each CSV source (URL/ref) via block-utils, merges with the pure core,
//! returns the merged CSV as a text `{result}`. `Input::None` + a required
//! `files` source_list (like merge-pdf). Surfaces: chat + CLI (no page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{respond_ok, Input, Param, SkillError, SourceFields, ToolDescriptor};
use gizza_ai_csv_merge_core::merge;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::source_list("files", 2).required().describe("Two or more CSV sources to concatenate. Each item has exactly one of `url` or `ref`."))
        .param(Param::boolean("header").default(true).describe("Each file has a header row; the output uses the union of all headers and aligns rows by column name. false = stack rows positionally. Default true."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvMerge;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Concatenate multiple CSVs into one",
    requires = ["wafer-run/network"],
    skill(
        description = "Concatenate (stack) two or more CSV files into one. With header=true (default) the output uses the UNION of all input headers and aligns each file's rows by column name (missing columns blank); with header=false rows are stacked positionally. Each file is a URL or a `ref` to an uploaded attachment. (Key-join is not yet supported — this concatenates.)",
        parameters = schema_json()
    ),
)]
impl CsvMerge {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid csv-merge args: {e}")))?;
    if args.files.len() < 2 {
        return Err(SkillError::InvalidArgs(format!("csv-merge needs at least 2 files, got {}", args.files.len())));
    }
    let mut texts: Vec<String> = Vec::with_capacity(args.files.len());
    for field in args.files {
        let (bytes, _mime, _name) = resolve_source(field.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
        texts.push(String::from_utf8(bytes).map_err(|e| SkillError::InvalidArgs(format!("a CSV input is not valid UTF-8: {e}")))?);
    }
    let delim = if args.delimiter.is_empty() { ",".to_string() } else { args.delimiter };
    let merged = merge(&texts, args.header, &delim).map_err(SkillError::InvalidArgs)?;
    respond_ok(&merged)
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
                    "files": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Two or more CSV sources to concatenate. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "header":    { "type": "boolean", "default": true, "description": "Each file has a header row; the output uses the union of all headers and aligns rows by column name. false = stack rows positionally. Default true." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["files"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

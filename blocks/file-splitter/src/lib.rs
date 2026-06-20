//! gizza-ai/file-splitter — fetch a text/CSV file (URL or attachment ref), split
//! it along line boundaries into parts / fixed line-count / fixed byte-size
//! pieces, and return the pieces bundled in a ZIP.
//!
//! Pipeline: resolve the source → `core::split` (pure, line-boundary) → bundle
//! pieces into a ZIP → base64 envelope. The `for_llm` text reports the count and
//! piece names.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP output fits neither the pure-text nor
//! the ffmpeg media page shape — the F3 no-page file-input pattern).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB text input

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_mode")]
    mode: String,
    count: u64,
    #[serde(default)]
    csv_header: bool,
}
fn default_mode() -> String {
    "parts".to_string()
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`. `mode` selects the split strategy;
/// `count` is its parameter; `csv_header` repeats a CSV header in each piece.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("mode", ["parts", "lines", "bytes"])
                .default("parts")
                .describe("How to split: 'parts' = count equal parts, 'lines' = count lines per piece, 'bytes' = up to count bytes per piece. Always splits on line boundaries."),
        )
        .param(
            Param::integer("count")
                .required()
                .min(1.0)
                .describe("The split parameter: number of parts (parts), lines per piece (lines), or max bytes per piece (bytes)."),
        )
        .param(
            Param::boolean("csv_header")
                .default(false)
                .describe("Treat the first line as a CSV header and repeat it at the top of every piece. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileSplitter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a text/CSV file into parts or chunks (ZIP of pieces)",
    requires = ["wafer-run/network"],
    skill(
        description = "Split a large text or CSV file into pieces, returned as a ZIP. Set mode='parts' (count equal parts), 'lines' (count lines each), or 'bytes' (up to count bytes each), with count as the parameter. Splitting is always on line boundaries (no line is cut). Set csv_header=true to repeat the first line as a header atop every piece. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl FileSplitter {
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
    use std::io::{Cursor, Write};
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    let args: Args = serde_json::from_slice(&body).invalid_args("file-splitter")?;
    let mode = gizza_ai_file_splitter_core::parse_mode(&args.mode)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid file-splitter args: {e}")))?;
    let count = usize::try_from(args.count)
        .map_err(|_| SkillError::InvalidArgs("count is too large".to_string()))?;

    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| SkillError::InvalidArgs("file is not valid UTF-8 text".to_string()))?;

    let pieces = gizza_ai_file_splitter_core::split(&text, mode, count, args.csv_header)
        .map_err(SkillError::InvalidArgs)?;

    // Output piece extension + base name from the input filename.
    let (stem, ext) = match in_filename.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && e.len() <= 8 => (s.to_string(), e.to_string()),
        _ => (
            if in_filename.is_empty() { "file".to_string() } else { in_filename.clone() },
            "txt".to_string(),
        ),
    };

    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (i, piece) in pieces.iter().enumerate() {
            let name = format!("{stem}-part-{:03}.{ext}", i + 1);
            zw.start_file(&name, opts)
                .map_err(|e| SkillError::Serialize(format!("zip start_file: {e}")))?;
            zw.write_all(piece.as_bytes())
                .map_err(|e| SkillError::Serialize(format!("zip write: {e}")))?;
        }
        zw.finish()
            .map_err(|e| SkillError::Serialize(format!("zip finish: {e}")))?;
    }

    let filename = format!("{stem}-split.zip");
    let zip_len = buf.len();
    let data_url = format!("data:application/zip;base64,{}", B64.encode(&buf));
    let for_llm = format!(
        "split {in_filename} into {} piece(s) ({} mode, count={count}) → {filename} ({zip_len}-byte ZIP)",
        pieces.len(),
        args.mode
    );

    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/zip".to_string(), filename },
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
                    "url":        { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode":       { "type": "string", "enum": ["parts", "lines", "bytes"], "default": "parts", "description": "How to split: 'parts' = count equal parts, 'lines' = count lines per piece, 'bytes' = up to count bytes per piece. Always splits on line boundaries." },
                    "count":      { "type": "integer", "minimum": 1, "description": "The split parameter: number of parts (parts), lines per piece (lines), or max bytes per piece (bytes)." },
                    "csv_header": { "type": "boolean", "default": false, "description": "Treat the first line as a CSV header and repeat it at the top of every piece. Default false." }
                },
                "additionalProperties": false,
                "required": ["count"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

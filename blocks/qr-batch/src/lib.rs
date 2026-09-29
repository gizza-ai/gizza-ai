//! gizza-ai/qr-batch — generate many QR codes from pasted rows and return a ZIP.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_qr_batch_core::{Columns, Ecc, InputFormat, Options, OutFormat};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    data: String,
    input_format: String,
    columns: String,
    has_header: bool,
    format: String,
    size: u32,
    margin: u32,
    error_correction: String,
    fg_color: String,
    bg_color: String,
    name_prefix: String,
    include_index: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data: String::new(),
            input_format: "auto".to_string(),
            columns: "auto".to_string(),
            has_header: false,
            format: "png".to_string(),
            size: 512,
            margin: 4,
            error_correction: "M".to_string(),
            fg_color: "#000000".to_string(),
            bg_color: "#ffffff".to_string(),
            name_prefix: "qr".to_string(),
            include_index: true,
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("Rows to encode. Paste one QR payload per line, or CSV/TSV rows such as `label,https://example.com`. Blank lines are ignored; at most 500 rows are accepted."))
        .param(Param::enumv("input_format", ["auto", "list", "csv", "tsv"]).default("auto").describe("How to split the pasted rows. `auto` (default) uses TSV if any tab is present, CSV if any comma is present, otherwise one value per line."))
        .param(Param::enumv("columns", ["auto", "name-value", "value-name", "value-only"]).default("auto").describe("Which columns to read when the input is CSV/TSV. `auto` treats two-column rows as name,value and one-column rows as values; `value-only` keeps commas/tabs inside the payload."))
        .param(Param::boolean("has_header").default(false).describe("Skip the first non-blank row as a header row. Default false."))
        .param(Param::enumv("format", ["png", "svg", "both"]).default("png").describe("File type to place in the ZIP for each row: PNG, SVG, or both. Default png."))
        .param(Param::integer("size").default(512).min(64.0).max(2048.0).describe("Target edge size in pixels for each QR image (64-2048, default 512). PNG output snaps upward to whole QR modules so edges stay sharp."))
        .param(Param::integer("margin").default(4).min(0.0).max(16.0).describe("Quiet-zone border in QR modules (0-16, default 4). Use at least 4 for printed codes unless you know the scanner tolerates less."))
        .param(Param::enumv("error_correction", ["L", "M", "Q", "H"]).default("M").describe("QR error-correction level. L fits the most data; M is the balanced default; Q and H survive more print damage but reduce capacity."))
        .param(Param::string("fg_color").default("#000000").describe("Foreground/module colour as #rgb, #rrggbb, or a common colour name. It cannot be transparent. Default #000000."))
        .param(Param::string("bg_color").default("#ffffff").describe("Background colour as #rgb, #rrggbb, a common colour name, or `transparent`. Default #ffffff."))
        .param(Param::string("name_prefix").default("qr").describe("Prefix for auto-numbered filenames when a row has no explicit name. `qr` becomes qr-001.png, qr-002.png, and so on."))
        .param(Param::boolean("include_index").default(true).describe("Include index.csv in the ZIP, mapping each generated filename back to the encoded value and listing row-level errors. Default true."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct QrBatch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/qr-batch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a ZIP of QR codes from a pasted list or CSV",
    skill(
        description = "Generate many QR codes at once from a pasted list, CSV or TSV and return a downloadable ZIP archive. Each row becomes a PNG, SVG or both; optional filename columns, header skipping, QR error correction, colours, size, quiet-zone margin and an index.csv manifest are supported. Row errors are reported instead of silently dropped, and generation is deterministic and local.",
        parameters = schema_json()
    ),
)]
impl QrBatch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("qr-batch")?;
    let batch = gizza_ai_qr_batch_core::generate_batch(&args.data, &options(&args)?)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &batch.zip,
        "application/zip",
        "qr-batch.zip".to_string(),
        batch.summary(),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(target_arch = "wasm32")]
fn options(args: &Args) -> Result<Options, SkillError> {
    Ok(Options {
        input_format: InputFormat::parse(&args.input_format).map_err(SkillError::InvalidArgs)?,
        columns: Columns::parse(&args.columns).map_err(SkillError::InvalidArgs)?,
        has_header: args.has_header,
        format: OutFormat::parse(&args.format).map_err(SkillError::InvalidArgs)?,
        size: args.size,
        margin: args.margin,
        ecc: Ecc::parse(&args.error_correction).map_err(SkillError::InvalidArgs)?,
        fg_color: args.fg_color.clone(),
        bg_color: args.bg_color.clone(),
        name_prefix: args.name_prefix.clone(),
        include_index: args.include_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived
            .get("properties")
            .and_then(|v| v.as_object())
            .unwrap();
        assert_eq!(derived["required"], serde_json::json!(["data"]));
        assert_eq!(derived["additionalProperties"], false);
        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["auto", "list", "csv", "tsv"])
        );
        assert_eq!(
            props["columns"]["enum"],
            serde_json::json!(["auto", "name-value", "value-name", "value-only"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["png", "svg", "both"])
        );
        assert_eq!(
            props["error_correction"]["enum"],
            serde_json::json!(["L", "M", "Q", "H"])
        );
        assert_eq!(props["size"]["default"], 512);
        assert_eq!(props["size"]["minimum"], 64.0);
        assert_eq!(props["size"]["maximum"], 2048.0);
        assert_eq!(props["margin"]["default"], 4);
        assert_eq!(props["has_header"]["default"], false);
        assert_eq!(props["include_index"]["default"], true);
        for (name, prop) in props {
            assert!(
                prop.get("description")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty()),
                "{name} missing description"
            );
        }
    }
}

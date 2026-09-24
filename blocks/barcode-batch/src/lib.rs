//! gizza-ai/barcode-batch — generate many 1D barcodes from pasted rows and
//! return a ZIP of images or one printable PDF label sheet.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_barcode_batch_core::{
    Columns, InputFormat, OutFormat, Options, Output, SheetPreset, Symbology,
};
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
    symbology: String,
    auto_check_digit: bool,
    output: String,
    format: String,
    sheet_preset: String,
    module_width: u32,
    bar_height: u32,
    quiet_zone: u32,
    show_text: bool,
    text_size: u32,
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
            symbology: "code128".to_string(),
            auto_check_digit: true,
            output: "zip".to_string(),
            format: "png".to_string(),
            sheet_preset: "avery-5160".to_string(),
            module_width: 2,
            bar_height: 100,
            quiet_zone: 10,
            show_text: true,
            text_size: 20,
            fg_color: "#000000".to_string(),
            bg_color: "#ffffff".to_string(),
            name_prefix: "barcode".to_string(),
            include_index: true,
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("Rows to encode. Paste one barcode value per line, or CSV/TSV rows such as `SKU-1001,widget`. Blank lines are ignored; at most 500 rows and 120 characters per value are accepted."))
        .param(Param::enumv("input_format", ["auto", "list", "csv", "tsv"]).default("auto").describe("How to split the pasted rows. `auto` (default) uses TSV if any tab is present, CSV if any comma is present, otherwise one value per line."))
        .param(Param::enumv("columns", ["auto", "value-name", "name-value", "value-only"]).default("auto").describe("Which columns to read when the input is CSV/TSV. `auto` and `value-name` treat two-column rows as value,filename; `name-value` reverses them; `value-only` keeps commas/tabs inside the barcode value."))
        .param(Param::boolean("has_header").default(false).describe("Skip the first non-blank row as a header row. Default false."))
        .param(Param::enumv("symbology", ["auto", "code128", "code39", "code93", "ean13", "ean8", "upca", "itf", "codabar"]).default("code128").describe("Barcode symbology for every row. `code128` (default) encodes any printable ASCII and picks the double-density digit subset automatically. `auto` chooses per row by shape: 14 digits ITF-14, 13 EAN-13, 12 UPC-A, 8 EAN-8, anything else Code 128."))
        .param(Param::boolean("auto_check_digit").default(true).describe("For EAN-13, EAN-8, UPC-A and ITF, compute the trailing mod-10 check digit when the pasted value is one digit short, and verify it when it is present. Default true. Turn off to reject short values instead."))
        .param(Param::enumv("output", ["zip", "sheet"]).default("zip").describe("What to bundle the batch as. `zip` (default) returns one image file per row plus index.csv; `sheet` returns a single printable PDF laid out on the label grid chosen by sheet_preset."))
        .param(Param::enumv("format", ["png", "svg", "both"]).default("png").describe("File type placed in the ZIP for each row: PNG, SVG, or both. Ignored when output is `sheet` (the PDF is always vector). Default png."))
        .param(Param::enumv("sheet_preset", ["avery-5160", "avery-5161", "avery-5163", "avery-l7651", "avery-l7160", "a4-grid"]).default("avery-5160").describe("Label-sheet grid used when output is `sheet`. avery-5160 = US Letter 30-up (2.625x1 in, default), 5161 = Letter 20-up (4x1 in), 5163 = Letter 10-up (4x2 in), l7651 = A4 65-up (38.1x21.2 mm), l7160 = A4 21-up (63.5x38.1 mm), a4-grid = A4 40-up (45x25 mm). Each label auto-fits its symbol."))
        .param(Param::integer("module_width").default(2).min(1.0).max(10.0).describe("Width in pixels of the narrowest bar (1-10, default 2). Raise it for low-resolution thermal printers. Ignored for `sheet`, where bars scale to the label."))
        .param(Param::integer("bar_height").default(100).min(20.0).max(400.0).describe("Bar height in pixels (20-400, default 100). Ignored for `sheet`, where bars fill the label height."))
        .param(Param::integer("quiet_zone").default(10).min(0.0).max(30.0).describe("Blank margin either side of the symbol, in modules (0-30, default 10). Code 128 and ITF need at least 10 to scan reliably; do not go below it for printed codes."))
        .param(Param::boolean("show_text").default(true).describe("Print the human-readable value under the bars, including any check digit that was computed. Default true."))
        .param(Param::integer("text_size").default(20).min(8.0).max(48.0).describe("Human-readable text height in pixels (8-48, default 20). PNG output snaps it to a whole multiple of the 8-pixel bitmap font."))
        .param(Param::string("fg_color").default("#000000").describe("Bar colour as #rgb, #rrggbb, or a common colour name. It cannot be transparent. Default #000000 — scanners need dark bars on a light background."))
        .param(Param::string("bg_color").default("#ffffff").describe("Background colour as #rgb, #rrggbb, a common colour name, or `transparent`. Default #ffffff."))
        .param(Param::string("name_prefix").default("barcode").describe("Prefix for auto-numbered filenames when a row has no filename column. `barcode` becomes barcode-001.png, barcode-002.png, and so on."))
        .param(Param::boolean("include_index").default(true).describe("Include index.csv in the ZIP, mapping each generated filename back to the encoded value and symbology and listing row-level errors. Default true; ignored for `sheet` output."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BarcodeBatch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/barcode-batch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a ZIP or printable PDF sheet of 1D barcodes from a list or CSV",
    skill(
        description = "Generate many 1D barcodes at once from a pasted list, CSV or TSV. Supports Code 128, Code 39, Code 93, EAN-13, EAN-8, UPC-A, ITF-14 and Codabar, with per-row symbology auto-detection and automatic mod-10 check-digit calculation. Returns either a ZIP of PNG/SVG files plus an index.csv manifest, or a single printable PDF label sheet on an Avery-style grid. Bar width, bar height, quiet zone, colours and human-readable text are configurable. Row errors are reported instead of silently dropped, and generation is deterministic and local.",
        parameters = schema_json()
    ),
)]
impl BarcodeBatch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("barcode-batch")?;
    let batch = gizza_ai_barcode_batch_core::generate_batch(&args.data, &options(&args)?)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &batch.bytes,
        batch.mime,
        batch.filename.to_string(),
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
        symbology: Symbology::parse(&args.symbology).map_err(SkillError::InvalidArgs)?,
        auto_check_digit: args.auto_check_digit,
        output: Output::parse(&args.output).map_err(SkillError::InvalidArgs)?,
        format: OutFormat::parse(&args.format).map_err(SkillError::InvalidArgs)?,
        sheet_preset: SheetPreset::parse(&args.sheet_preset).map_err(SkillError::InvalidArgs)?,
        module_width: args.module_width,
        bar_height: args.bar_height,
        quiet_zone: args.quiet_zone,
        show_text: args.show_text,
        text_size: args.text_size,
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
            serde_json::json!(["auto", "value-name", "name-value", "value-only"])
        );
        assert_eq!(
            props["symbology"]["enum"],
            serde_json::json!([
                "auto", "code128", "code39", "code93", "ean13", "ean8", "upca", "itf", "codabar"
            ])
        );
        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["zip", "sheet"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["png", "svg", "both"])
        );
        assert_eq!(
            props["sheet_preset"]["enum"],
            serde_json::json!([
                "avery-5160",
                "avery-5161",
                "avery-5163",
                "avery-l7651",
                "avery-l7160",
                "a4-grid"
            ])
        );
        assert_eq!(props["symbology"]["default"], "code128");
        assert_eq!(props["output"]["default"], "zip");
        assert_eq!(props["sheet_preset"]["default"], "avery-5160");
        assert_eq!(props["module_width"]["default"], 2);
        assert_eq!(props["module_width"]["minimum"], 1.0);
        assert_eq!(props["module_width"]["maximum"], 10.0);
        assert_eq!(props["bar_height"]["default"], 100);
        assert_eq!(props["bar_height"]["maximum"], 400.0);
        assert_eq!(props["quiet_zone"]["default"], 10);
        assert_eq!(props["text_size"]["default"], 20);
        assert_eq!(props["has_header"]["default"], false);
        assert_eq!(props["auto_check_digit"]["default"], true);
        assert_eq!(props["show_text"]["default"], true);
        assert_eq!(props["include_index"]["default"], true);
        assert_eq!(props["fg_color"]["default"], "#000000");
        assert_eq!(props["bg_color"]["default"], "#ffffff");
        assert_eq!(props["name_prefix"]["default"], "barcode");
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

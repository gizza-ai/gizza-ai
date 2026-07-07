//! gizza-ai/qif-ofx-convert — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page + query-params); handle() delegates to block_utils::run_skill.
//! Pure compute — no host calls, runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_qif_ofx_convert_core::{convert, DateFormat, Format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    date_format: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    invert_amounts: bool,
    #[serde(default)]
    drop_empty_columns: bool,
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The QIF or OFX/QFX bank-export text to convert. Paste the whole file contents (QIF records ending in '^', or OFX <STMTTRN> blocks)."),
        )
        .param(
            Param::enumv("format", ["auto", "qif", "ofx"])
                .default("auto")
                .describe("Input format. 'auto' (default) detects it: an OFX marker (<OFX>, <STMTTRN>, or OFXHEADER) selects OFX, otherwise QIF. 'ofx' also accepts QFX."),
        )
        .param(
            Param::enumv("date_format", ["iso", "us", "eu", "raw"])
                .default("iso")
                .describe("How the Date column is written. iso=YYYY-MM-DD (default), us=MM/DD/YYYY, eu=DD/MM/YYYY, raw=keep the source date text unchanged. QIF month/day order is auto-detected (US month-first unless the first field exceeds 12)."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("Output CSV field separator. Default comma. Use semicolon or a comma-decimal locale, tab for TSV, or pipe."),
        )
        .param(
            Param::boolean("invert_amounts")
                .default(false)
                .describe("Flip the sign of every amount — for banks/apps that export debits as positive and credits as negative (or vice-versa). Default false (signs kept as in the source)."),
        )
        .param(
            Param::boolean("drop_empty_columns")
                .default(false)
                .describe("Omit output columns that hold no data across all rows (e.g. Type/FITID for QIF, or Category for OFX). Default false keeps the full fixed 8-column set (Date, Amount, Payee, Memo, Category, Check Number, Type, FITID) so import templates stay stable."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct QifOfxConvert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/qif-ofx-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert QIF and OFX/QFX bank exports into clean normalized CSV.",
    skill(
        description = "Convert a QIF (Quicken Interchange Format) or OFX/QFX (Open Financial Exchange, both 1.x SGML and 2.x XML) bank export into a single normalized CSV with columns Date, Amount, Payee, Memo, Category, Check Number, Type, FITID — ready to import into budgeting/spreadsheet apps. format='auto' (default) detects QIF vs OFX; date_format normalizes the Date column to iso/us/eu (or raw); delimiter picks the CSV separator (comma/semicolon/tab/pipe); invert_amounts flips every amount's sign; drop_empty_columns omits columns that are empty across all rows. Paste the file contents in the 'data' field. Runs entirely in-browser; nothing is uploaded.",
        parameters = schema_json()
    )
)]
impl QifOfxConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "qif-ofx-convert", |a: Args| {
            let fmt = Format::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let df = DateFormat::parse(&a.date_format).map_err(SkillError::InvalidArgs)?;
            convert(
                &a.data,
                fmt,
                df,
                &a.delimiter,
                a.invert_amounts,
                a.drop_empty_columns,
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
    /// reviewed. (Regenerate this literal when the descriptor changes -- see
    /// /improve-tool reference.md "drift-guard".)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The QIF or OFX/QFX bank-export text to convert. Paste the whole file contents (QIF records ending in '^', or OFX <STMTTRN> blocks)." },
                    "format": { "type": "string", "enum": ["auto", "qif", "ofx"], "default": "auto", "description": "Input format. 'auto' (default) detects it: an OFX marker (<OFX>, <STMTTRN>, or OFXHEADER) selects OFX, otherwise QIF. 'ofx' also accepts QFX." },
                    "date_format": { "type": "string", "enum": ["iso", "us", "eu", "raw"], "default": "iso", "description": "How the Date column is written. iso=YYYY-MM-DD (default), us=MM/DD/YYYY, eu=DD/MM/YYYY, raw=keep the source date text unchanged. QIF month/day order is auto-detected (US month-first unless the first field exceeds 12)." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Output CSV field separator. Default comma. Use semicolon or a comma-decimal locale, tab for TSV, or pipe." },
                    "invert_amounts": { "type": "boolean", "default": false, "description": "Flip the sign of every amount — for banks/apps that export debits as positive and credits as negative (or vice-versa). Default false (signs kept as in the source)." },
                    "drop_empty_columns": { "type": "boolean", "default": false, "description": "Omit output columns that hold no data across all rows (e.g. Type/FITID for QIF, or Category for OFX). Default false keeps the full fixed 8-column set (Date, Amount, Payee, Memo, Category, Check Number, Type, FITID) so import templates stay stable." }
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

//! gizza-ai/ics-to-csv — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page + query-params); handle() delegates to block_utils::run_skill.
//! Pure compute — no host calls, runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ics_to_csv_core::convert_str;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ics: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    date_format: String,
    #[serde(default = "default_true")]
    include_location: bool,
    #[serde(default = "default_true")]
    include_description: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("ics")
                .required()
                .describe("The iCalendar (.ics) document to convert. Paste the whole file contents — it starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks. Folded lines are unfolded and RFC 5545 TEXT escapes are decoded."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("Output CSV field separator. Default comma. Use semicolon for comma-decimal locales, tab for TSV, or pipe."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit the column-name header row. Default true; set false for a headerless CSV."),
        )
        .param(
            Param::enumv("date_format", ["iso", "raw", "unix"])
                .default("iso")
                .describe("How the start/end date columns are written. iso (default) normalizes to ISO-8601 (20240309T081530Z → 2024-03-09T08:15:30Z, 20240309 → 2024-03-09); raw keeps the original .ics value; unix converts to epoch seconds (Z times exact, floating/TZID read as UTC)."),
        )
        .param(
            Param::boolean("include_location")
                .default(true)
                .describe("Include the LOCATION column. Default true; set false to drop it."),
        )
        .param(
            Param::boolean("include_description")
                .default(true)
                .describe("Include the DESCRIPTION column. Default true; set false to drop it (handy when descriptions are long)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IcsToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ics-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten the events in an .ics calendar file into a clean CSV table.",
    skill(
        description = "Convert an iCalendar (.ics) document into a CSV table with one row per VEVENT — columns summary, start, end, location, and description, plus status, categories, and uid columns whenever any event carries them. delimiter picks the separator (comma/semicolon/tab/pipe); header toggles the header row; date_format writes the start/end columns as iso (normalized ISO-8601), raw (original .ics text), or unix (epoch seconds); include_location and include_description drop those columns. Folded lines are unfolded, RFC 5545 TEXT escapes (\\n, \\,, \\;) are decoded, VALARM/VTIMEZONE sub-components are skipped, and every field is RFC-4180 escaped. Paste the .ics contents in the 'ics' field. Runs entirely in-browser; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl IcsToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ics-to-csv", |a: Args| {
            convert_str(
                &a.ics,
                &a.delimiter,
                a.header,
                &a.date_format,
                a.include_location,
                a.include_description,
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
    /// reviewed. (Regenerate this literal when the descriptor changes.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "ics": { "type": "string", "description": "The iCalendar (.ics) document to convert. Paste the whole file contents — it starts with BEGIN:VCALENDAR and contains one or more BEGIN:VEVENT…END:VEVENT blocks. Folded lines are unfolded and RFC 5545 TEXT escapes are decoded." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Output CSV field separator. Default comma. Use semicolon for comma-decimal locales, tab for TSV, or pipe." },
                    "header": { "type": "boolean", "default": true, "description": "Emit the column-name header row. Default true; set false for a headerless CSV." },
                    "date_format": { "type": "string", "enum": ["iso", "raw", "unix"], "default": "iso", "description": "How the start/end date columns are written. iso (default) normalizes to ISO-8601 (20240309T081530Z → 2024-03-09T08:15:30Z, 20240309 → 2024-03-09); raw keeps the original .ics value; unix converts to epoch seconds (Z times exact, floating/TZID read as UTC)." },
                    "include_location": { "type": "boolean", "default": true, "description": "Include the LOCATION column. Default true; set false to drop it." },
                    "include_description": { "type": "boolean", "default": true, "description": "Include the DESCRIPTION column. Default true; set false to drop it (handy when descriptions are long)." }
                },
                "required": ["ics"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

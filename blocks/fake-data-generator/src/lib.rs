//! gizza-ai/fake-data-generator — generate rows of realistic-looking fake test
//! records (names, emails, phones, addresses, and more) as CSV or JSON.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Deterministic given a non-zero
//! `seed`; with the default seed=0 the generator uses a fixed reproducible
//! fallback (each surface may pass its own time-derived seed to vary output).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default)]
    format: String,
    #[serde(default)]
    fields: String,
    #[serde(default)]
    seed: u64,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    table: String,
}

fn default_count() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::integer("count")
                .default(10)
                .min(1.0)
                .max(gizza_ai_fake_data_generator_core::MAX_ROWS as f64)
                .describe("Number of fake records to generate, 1-1000. Default 10."),
        )
        .param(
            Param::enumv("format", ["csv", "json", "sql", "xml"])
                .default("csv")
                .describe("Output format: 'csv' (default) emits an optional header row plus data rows; 'json' a pretty-printed array of objects; 'sql' INSERT statements; 'xml' a <records> document."),
        )
        .param(
            Param::string("fields")
                .describe("Comma-separated columns to include, in any order. Valid: id, uuid, first_name, last_name, full_name, username, email, phone, street, city, state, zip, country, latitude, longitude, company, job_title, birthdate, domain, ipv4, ipv6, mac, color, boolean, credit_card, sentence. Use 'all' for every field. Blank → full_name, email, phone, street, city, state, zip."),
        )
        .param(
            Param::integer("seed")
                .default(0)
                .min(0.0)
                .describe("Reproducibility seed. A non-zero value always reproduces the same dataset; 0 (default) uses a fixed fallback seed."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("CSV only: include the column-name header row. Default true."),
        )
        .param(
            Param::string("table")
                .describe("SQL only: table name for the INSERT statements. Blank → 'people'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fake-data-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate fake test records (names, emails, phones, addresses, IDs) as CSV, JSON, SQL, or XML",
    skill(
        description = "Generate rows of realistic-looking but entirely fake test/sample data — names, emails, phone numbers, postal addresses, usernames, companies, job titles, birthdates, UUIDs, IP/MAC addresses, colors, booleans, credit-card-style numbers, lorem sentences, and more — as CSV, JSON, SQL INSERT statements, or XML. Set count for the number of rows (1-1000), format='csv'|'json'|'sql'|'xml', and fields to a comma-separated subset of columns (id, uuid, first_name, last_name, full_name, username, email, phone, street, city, state, zip, country, latitude, longitude, company, job_title, birthdate, domain, ipv4, ipv6, mac, color, boolean, credit_card, sentence) or 'all'. header toggles the CSV header row; table sets the SQL table name. All data is synthetic placeholder data for testing/mocking, not real people; credit_card numbers are Luhn-valid test placeholders, not real accounts. Pass a non-zero seed to reproduce the exact same dataset.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fake-data-generator", |a: Args| {
            // Chat: derive a varying seed from the wall clock when the caller
            // leaves seed=0, so repeated calls aren't identical; a supplied
            // non-zero seed always reproduces the dataset.
            let seed = if a.seed == 0 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0)
            } else {
                a.seed
            };
            gizza_ai_fake_data_generator_core::generate_opts(
                a.count, &a.format, &a.fields, seed, a.header, &a.table,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "count": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 10, "description": "Number of fake records to generate, 1-1000. Default 10." },
                    "format": { "type": "string", "enum": ["csv", "json", "sql", "xml"], "default": "csv", "description": "Output format: 'csv' (default) emits an optional header row plus data rows; 'json' a pretty-printed array of objects; 'sql' INSERT statements; 'xml' a <records> document." },
                    "fields": { "type": "string", "description": "Comma-separated columns to include, in any order. Valid: id, uuid, first_name, last_name, full_name, username, email, phone, street, city, state, zip, country, latitude, longitude, company, job_title, birthdate, domain, ipv4, ipv6, mac, color, boolean, credit_card, sentence. Use 'all' for every field. Blank → full_name, email, phone, street, city, state, zip." },
                    "seed": { "type": "integer", "minimum": 0, "default": 0, "description": "Reproducibility seed. A non-zero value always reproduces the same dataset; 0 (default) uses a fixed fallback seed." },
                    "header": { "type": "boolean", "default": true, "description": "CSV only: include the column-name header row. Default true." },
                    "table": { "type": "string", "description": "SQL only: table name for the INSERT statements. Blank → 'people'." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

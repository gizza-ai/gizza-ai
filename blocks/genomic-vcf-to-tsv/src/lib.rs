//! gizza-ai/genomic-vcf-to-tsv — chat skill block on the shared tool abstraction.
//! Flattens VCF records into TSV with exploded INFO and optional sample FORMAT fields.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    layout: String,
    #[serde(default = "default_true")]
    include_info: bool,
    #[serde(default = "default_true")]
    include_samples: bool,
    #[serde(default)]
    info_fields: String,
    #[serde(default)]
    pass_only: bool,
    #[serde(default)]
    prefix_info: bool,
    #[serde(default = "default_missing")]
    missing: String,
    #[serde(default = "default_true")]
    header: bool,
}

fn default_true() -> bool {
    true
}
fn default_missing() -> String {
    ".".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("VCF text to flatten. Paste tab-delimited Variant Call Format records, including optional ## metadata lines and a #CHROM header. Supports fixed VCF columns, INFO key/value pairs or flags, and sample FORMAT columns."))
        .param(Param::enumv("layout", ["long", "wide"]).default("long").describe("How sample genotype columns are emitted. 'long' (default) outputs one row per variant per sample with a SAMPLE column; 'wide' outputs one row per variant with <sample>_<FORMATKEY> columns."))
        .param(Param::boolean("include_info").default(true).describe("Explode the INFO column into one TSV column per discovered INFO key. Default true."))
        .param(Param::boolean("include_samples").default(true).describe("Include per-sample FORMAT/genotype values when the VCF has sample columns. Default true."))
        .param(Param::string("info_fields").default("").describe("Optional comma-separated whitelist of INFO keys to keep, in the requested order (for example 'DP,AF,AC'). Leave blank to include every discovered INFO key."))
        .param(Param::boolean("pass_only").default(false).describe("When true, keep only records whose FILTER is PASS or '.' and drop filtered calls such as q10. Default false."))
        .param(Param::boolean("prefix_info").default(false).describe("Prefix exploded INFO columns with INFO_ to avoid collisions with FORMAT keys such as DP. Default false."))
        .param(Param::string("missing").default(".").describe("Placeholder written when an INFO key, FORMAT key, or sample value is absent. Default '.'."))
        .param(Param::boolean("header").default(true).describe("Emit the TSV header row. Turn off for append-only pipelines that already have column names. Default true."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/genomic-vcf-to-tsv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten VCF variant records into tidy TSV with exploded INFO and optional sample genotype fields.",
    skill(
        description = "Flatten genomic Variant Call Format (VCF 4.x) text into a tidy tab-separated table. The fixed CHROM/POS/ID/REF/ALT/QUAL/FILTER columns lead every row; INFO key/value pairs are expanded into columns; sample FORMAT/genotype fields can be emitted in long layout (one row per variant × sample) or wide layout (one row per variant with <sample>_<FORMATKEY> columns). Use info_fields to keep only selected INFO keys, pass_only to keep PASS/unfiltered calls, prefix_info to disambiguate INFO columns, missing to choose the empty-value placeholder, and header=false for no header row. Runs locally and does not validate reference genomes or normalize variants.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "genomic-vcf-to-tsv", |a: Args| {
            gizza_ai_genomic_vcf_to_tsv_core::run(
                &a.input,
                &a.layout,
                a.include_info,
                a.include_samples,
                &a.info_fields,
                a.pass_only,
                a.prefix_info,
                &a.missing,
                a.header,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "input":{"type":"string","description":"VCF text to flatten. Paste tab-delimited Variant Call Format records, including optional ## metadata lines and a #CHROM header. Supports fixed VCF columns, INFO key/value pairs or flags, and sample FORMAT columns."},
                    "layout":{"type":"string","enum":["long","wide"],"default":"long","description":"How sample genotype columns are emitted. 'long' (default) outputs one row per variant per sample with a SAMPLE column; 'wide' outputs one row per variant with <sample>_<FORMATKEY> columns."},
                    "include_info":{"type":"boolean","default":true,"description":"Explode the INFO column into one TSV column per discovered INFO key. Default true."},
                    "include_samples":{"type":"boolean","default":true,"description":"Include per-sample FORMAT/genotype values when the VCF has sample columns. Default true."},
                    "info_fields":{"type":"string","default":"","description":"Optional comma-separated whitelist of INFO keys to keep, in the requested order (for example 'DP,AF,AC'). Leave blank to include every discovered INFO key."},
                    "pass_only":{"type":"boolean","default":false,"description":"When true, keep only records whose FILTER is PASS or '.' and drop filtered calls such as q10. Default false."},
                    "prefix_info":{"type":"boolean","default":false,"description":"Prefix exploded INFO columns with INFO_ to avoid collisions with FORMAT keys such as DP. Default false."},
                    "missing":{"type":"string","default":".","description":"Placeholder written when an INFO key, FORMAT key, or sample value is absent. Default '.'."},
                    "header":{"type":"boolean","default":true,"description":"Emit the TSV header row. Turn off for append-only pipelines that already have column names. Default true."}
                },
                "required":["input"],
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

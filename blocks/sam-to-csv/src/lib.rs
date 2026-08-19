//! gizza-ai/sam-to-csv — chat skill block on the shared tool abstraction.
//! Turns SAM alignment records into a delimited table with named columns,
//! optional decoded FLAG bits, computed span columns, and optional tag columns.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    flags: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    tag_fields: String,
    #[serde(default = "default_true")]
    include_seq: bool,
    #[serde(default)]
    computed: bool,
    #[serde(default)]
    mapped_only: bool,
    #[serde(default)]
    primary_only: bool,
    #[serde(default)]
    min_mapq: u32,
    #[serde(default = "default_missing")]
    missing: String,
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
        .param(Param::string("input").required().describe("SAM text to parse. Paste tab-separated Sequence Alignment/Map records; @HD/@SQ/@RG/@PG/@CO header lines are skipped. Each record needs the 11 mandatory fields QNAME, FLAG, RNAME, POS, MAPQ, CIGAR, RNEXT, PNEXT, TLEN, SEQ, QUAL, optionally followed by TAG:TYPE:VALUE fields such as NM:i:0."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Output field separator. 'comma' (default) writes CSV with RFC 4180 quoting; 'tab' writes TSV; 'semicolon' and 'pipe' suit European spreadsheets and shell pipelines."))
        .param(Param::boolean("header").default(true).describe("Emit the column-name header row. Turn off when appending to a table that already has one. Default true."))
        .param(Param::enumv("flags", ["none", "summary", "bits", "both"]).default("summary").describe("How the bitwise FLAG is decoded. 'summary' (default) adds one FLAG_SUMMARY column listing the set bit names (PAIRED, PROPER_PAIR, UNMAPPED, MATE_UNMAPPED, REVERSE, MATE_REVERSE, READ1, READ2, SECONDARY, QCFAIL, DUPLICATE, SUPPLEMENTARY); 'bits' adds 12 true/false columns FLAG_PAIRED..FLAG_SUPPLEMENTARY; 'both' adds all of them; 'none' keeps only the raw FLAG number."))
        .param(Param::enumv("tags", ["none", "joined", "expand"]).default("expand").describe("How optional TAG:TYPE:VALUE fields are emitted. 'expand' (default) creates one column per discovered tag name (NM, AS, MD, ...) holding its value; 'joined' puts them all in a single TAGS column as 'NM:1 AS:30'; 'none' drops them."))
        .param(Param::string("tag_fields").default("").describe("Optional comma-separated whitelist of tag names to keep, in the requested output order (for example 'NM,AS,MD'). Leave blank to keep every tag found."))
        .param(Param::boolean("include_seq").default(true).describe("Keep the SEQ and QUAL columns. Turn off for a compact coordinate table without read bases and base qualities. Default true."))
        .param(Param::boolean("computed").default(false).describe("Add columns derived from POS, CIGAR and SEQ: END (last reference base covered), REF_SPAN (reference bases consumed by M/D/N/=/X), READ_LEN (SEQ length, or the CIGAR query span when SEQ is '*'), and STRAND ('+' or '-' from the 0x10 bit). Default false."))
        .param(Param::boolean("mapped_only").default(false).describe("Keep only mapped records by dropping those with the 0x4 (UNMAPPED) bit set. Default false."))
        .param(Param::boolean("primary_only").default(false).describe("Keep only primary alignments by dropping records with the 0x100 (SECONDARY) or 0x800 (SUPPLEMENTARY) bit set. Default false."))
        .param(Param::integer("min_mapq").default(0).min(0.0).max(255.0).describe("Drop records whose MAPQ is below this value (0-255). MAPQ 255 means 'unavailable' and is never filtered out by a threshold below 255. Default 0 (keep everything)."))
        .param(Param::string("missing").default(".").describe("Placeholder written when a value does not apply: an absent tag, a FLAG of 0 in summary mode, or END/REF_SPAN/STRAND for an unmapped record. Default '.'."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sam-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse SAM alignment records into a CSV table with named columns and decoded FLAG bits.",
    skill(
        description = "Parse SAM (Sequence Alignment/Map) text into a delimited table with named columns. @ header lines are skipped and each alignment line becomes one row of QNAME, FLAG, RNAME, POS, MAPQ, CIGAR, RNEXT, PNEXT, TLEN, SEQ and QUAL. The bitwise FLAG can be decoded into a FLAG_SUMMARY column of set bit names, into 12 true/false FLAG_* columns, or both. Optional TAG:TYPE:VALUE fields can be expanded one column per tag, joined into a single TAGS column, or dropped, and tag_fields whitelists which tags to keep. Set computed=true to add END, REF_SPAN, READ_LEN and STRAND derived from POS/CIGAR/SEQ; use mapped_only, primary_only and min_mapq to filter records; include_seq=false drops the bulky SEQ/QUAL columns; delimiter picks comma, tab, semicolon or pipe output. Runs locally on pasted text: it does not read BAM/CRAM binaries, index files, or a reference genome.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sam-to-csv", |a: Args| {
            gizza_ai_sam_to_csv_core::run(
                &a.input,
                &a.delimiter,
                a.header,
                &a.flags,
                &a.tags,
                &a.tag_fields,
                a.include_seq,
                a.computed,
                a.mapped_only,
                a.primary_only,
                a.min_mapq,
                &a.missing,
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
                    "input":{"type":"string","description":"SAM text to parse. Paste tab-separated Sequence Alignment/Map records; @HD/@SQ/@RG/@PG/@CO header lines are skipped. Each record needs the 11 mandatory fields QNAME, FLAG, RNAME, POS, MAPQ, CIGAR, RNEXT, PNEXT, TLEN, SEQ, QUAL, optionally followed by TAG:TYPE:VALUE fields such as NM:i:0."},
                    "delimiter":{"type":"string","enum":["comma","tab","semicolon","pipe"],"default":"comma","description":"Output field separator. 'comma' (default) writes CSV with RFC 4180 quoting; 'tab' writes TSV; 'semicolon' and 'pipe' suit European spreadsheets and shell pipelines."},
                    "header":{"type":"boolean","default":true,"description":"Emit the column-name header row. Turn off when appending to a table that already has one. Default true."},
                    "flags":{"type":"string","enum":["none","summary","bits","both"],"default":"summary","description":"How the bitwise FLAG is decoded. 'summary' (default) adds one FLAG_SUMMARY column listing the set bit names (PAIRED, PROPER_PAIR, UNMAPPED, MATE_UNMAPPED, REVERSE, MATE_REVERSE, READ1, READ2, SECONDARY, QCFAIL, DUPLICATE, SUPPLEMENTARY); 'bits' adds 12 true/false columns FLAG_PAIRED..FLAG_SUPPLEMENTARY; 'both' adds all of them; 'none' keeps only the raw FLAG number."},
                    "tags":{"type":"string","enum":["none","joined","expand"],"default":"expand","description":"How optional TAG:TYPE:VALUE fields are emitted. 'expand' (default) creates one column per discovered tag name (NM, AS, MD, ...) holding its value; 'joined' puts them all in a single TAGS column as 'NM:1 AS:30'; 'none' drops them."},
                    "tag_fields":{"type":"string","default":"","description":"Optional comma-separated whitelist of tag names to keep, in the requested output order (for example 'NM,AS,MD'). Leave blank to keep every tag found."},
                    "include_seq":{"type":"boolean","default":true,"description":"Keep the SEQ and QUAL columns. Turn off for a compact coordinate table without read bases and base qualities. Default true."},
                    "computed":{"type":"boolean","default":false,"description":"Add columns derived from POS, CIGAR and SEQ: END (last reference base covered), REF_SPAN (reference bases consumed by M/D/N/=/X), READ_LEN (SEQ length, or the CIGAR query span when SEQ is '*'), and STRAND ('+' or '-' from the 0x10 bit). Default false."},
                    "mapped_only":{"type":"boolean","default":false,"description":"Keep only mapped records by dropping those with the 0x4 (UNMAPPED) bit set. Default false."},
                    "primary_only":{"type":"boolean","default":false,"description":"Keep only primary alignments by dropping records with the 0x100 (SECONDARY) or 0x800 (SUPPLEMENTARY) bit set. Default false."},
                    "min_mapq":{"type":"integer","default":0,"minimum":0,"maximum":255,"description":"Drop records whose MAPQ is below this value (0-255). MAPQ 255 means 'unavailable' and is never filtered out by a threshold below 255. Default 0 (keep everything)."},
                    "missing":{"type":"string","default":".","description":"Placeholder written when a value does not apply: an absent tag, a FLAG of 0 in summary mode, or END/REF_SPAN/STRAND for an unmapped record. Default '.'."}
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

//! gizza-ai/zip-integrity-checker — test a .zip archive for corruption by
//! decompressing EVERY entry in memory and recomputing its CRC-32.
//!
//! Pipeline: resolve the source file → `core::verify` (pure-Rust zip + crc32fast,
//! nothing written to disk) → flat JSON the LLM reads directly: an `ok` verdict,
//! a one-line `summary`, per-entry `status` with the recorded AND recomputed
//! CRC-32, and archive-wide counts.
//!
//! Sibling `blocks/zip-inspect` LISTS an archive and reports the CRC-32 as
//! recorded; this one checks whether the data actually hashes to it. Detection
//! only — nothing is repaired.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (file→JSON, the no-page file-input pattern,
//! like unzip / zip-inspect / extract-tar / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_zip_integrity_checker_core::{
    verify, Report, ReportLevel, DEFAULT_MAX_UNCOMPRESSED_MB, MAX_MAX_UNCOMPRESSED_MB,
    MIN_MAX_UNCOMPRESSED_MB,
};
use serde::Deserialize;
use wafer_sdk::*;

/// Same 64 MiB archive cap as `unzip` — the sandbox memory budget, stated in the
/// parameter copy and in the error so a caller is never surprised by it.
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    report: Option<String>,
    #[serde(default)]
    max_uncompressed_mb: Option<u64>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("report", ["all", "problems"])
                .default("all")
                .describe(
                    "Which entries to list: 'all' (default) returns every entry with its verdict; \
                     'problems' returns only the entries that failed or could not be tested — the \
                     quiet mode for scripts. The verdict, counts and totals are identical either \
                     way; only the entries list is filtered.",
                ),
        )
        .param(
            Param::integer("max_uncompressed_mb")
                .default(DEFAULT_MAX_UNCOMPRESSED_MB as i64)
                .min(MIN_MAX_UNCOMPRESSED_MB as f64)
                .max(MAX_MAX_UNCOMPRESSED_MB as f64)
                .describe(
                    "Zip-bomb guard: the most data, in megabytes, this check will expand in total \
                     (1-4096, default 512). Testing means decompressing every entry, so a small \
                     archive can expand enormously; going over the budget is an error naming the \
                     size needed, not a pass. Raise it for a genuinely large archive.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ZipIntegrityChecker;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/zip-integrity-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Test a .zip archive for corruption by verifying every entry's CRC-32",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Test a .zip archive for corruption: decompress every entry in memory (nothing is written to disk) and compare the recomputed CRC-32 and byte count against what the archive records. Returns an overall ok verdict plus a one-line summary, and for each entry a status of ok, crc_mismatch (the data does not hash to the recorded checksum), size_mismatch (it expanded to a different number of bytes than recorded), data_error (the stream could not be decompressed — truncated or corrupt), unsupported_method (a compression method this build cannot decode), or encrypted (no password support). Both the recorded expected_crc32 and the recomputed actual_crc32 are reported side by side, along with method, sizes, the name of the first_bad_entry, per-status counts (checked/failed/skipped), archive totals, any prepended self-extracting stub size, and the archive comment. Entries that could not be tested are counted separately and never pass silently. Set report=problems for only the bad entries. Limits: ZIP only (not RAR/7z/tar), detection only — damaged archives are not repaired, encrypted entries need a password this tool does not accept, and the archive is capped at 64 MB with decompression bounded by max_uncompressed_mb. Provide the zip as either url (HTTP/HTTPS) or ref. Runs locally — the archive never leaves the device.",
        parameters = schema_json()
    ),
)]
impl ZipIntegrityChecker {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("zip-integrity-checker")?;
    let (level, budget_mb) = options(&args)?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let report = check(&bytes, level, budget_mb)?;
    let mut value =
        serde_json::to_value(&report).map_err(|e| SkillError::Serialize(e.to_string()))?;
    if !filename.is_empty() {
        if let Some(obj) = value.as_object_mut() {
            obj.insert("filename".into(), serde_json::Value::String(filename));
        }
    }
    serde_json::to_vec(&value).map_err(|e| {
        SkillError::Serialize(format!("serialize zip-integrity-checker response: {e}"))
    })
}

fn options(args: &Args) -> Result<(ReportLevel, u64), SkillError> {
    let level = ReportLevel::parse(args.report.as_deref().unwrap_or("all"))
        .map_err(SkillError::InvalidArgs)?;
    let budget_mb = args
        .max_uncompressed_mb
        .unwrap_or(DEFAULT_MAX_UNCOMPRESSED_MB)
        .clamp(MIN_MAX_UNCOMPRESSED_MB, MAX_MAX_UNCOMPRESSED_MB);
    Ok((level, budget_mb))
}

fn check(bytes: &[u8], level: ReportLevel, budget_mb: u64) -> Result<Report, SkillError> {
    if bytes.is_empty() {
        return Err(SkillError::InvalidArgs(
            "the archive is empty — provide a .zip file (up to 64 MB) as url or ref".into(),
        ));
    }
    verify(bytes, level, budget_mb).map_err(SkillError::InvalidArgs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "report": {
                        "type": "string",
                        "enum": ["all", "problems"],
                        "default": "all",
                        "description": "Which entries to list: 'all' (default) returns every entry with its verdict; 'problems' returns only the entries that failed or could not be tested — the quiet mode for scripts. The verdict, counts and totals are identical either way; only the entries list is filtered."
                    },
                    "max_uncompressed_mb": {
                        "type": "integer",
                        "default": 512,
                        "minimum": 1,
                        "maximum": 4096,
                        "description": "Zip-bomb guard: the most data, in megabytes, this check will expand in total (1-4096, default 512). Testing means decompressing every entry, so a small archive can expand enormously; going over the budget is an error naming the size needed, not a pass. Raise it for a genuinely large archive."
                    }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn defaults_are_applied_when_only_a_source_is_given() {
        let (level, mb) = options(&args(r#"{"url":"https://example.com/a.zip"}"#)).unwrap();
        assert_eq!(level, ReportLevel::All);
        assert_eq!(mb, DEFAULT_MAX_UNCOMPRESSED_MB);
    }

    #[test]
    fn parameters_are_parsed_and_clamped() {
        let (level, mb) = options(&args(
            r#"{"ref":"file_1","report":"problems","max_uncompressed_mb":999999}"#,
        ))
        .unwrap();
        assert_eq!(level, ReportLevel::Problems);
        assert_eq!(mb, MAX_MAX_UNCOMPRESSED_MB);

        let (_, small) = options(&args(r#"{"ref":"file_1","max_uncompressed_mb":0}"#)).unwrap();
        assert_eq!(small, MIN_MAX_UNCOMPRESSED_MB);
    }

    #[test]
    fn an_unknown_report_level_is_rejected() {
        let err = options(&args(
            r#"{"url":"https://e.example/a.zip","report":"quiet"}"#,
        ))
        .unwrap_err();
        assert!(
            format!("{err:?}").contains("unknown report level"),
            "{err:?}"
        );
    }

    #[test]
    fn an_empty_archive_is_rejected() {
        let err = check(&[], ReportLevel::All, 512).unwrap_err();
        assert!(format!("{err:?}").contains("archive is empty"), "{err:?}");
    }

    #[test]
    fn a_non_zip_file_is_rejected() {
        let err = check(b"just some prose, not an archive", ReportLevel::All, 512).unwrap_err();
        assert!(
            format!("{err:?}").contains("not a valid zip archive"),
            "{err:?}"
        );
    }
}

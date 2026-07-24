//! gizza-ai/dicom-anonymizer — strip patient-identifying (PHI) data elements
//! from a DICOM file and return a sanitized copy.
//!
//! Pipeline: resolve the source file (url/ref) → `core::anonymize` (pure,
//! Part-10 DICOM walker) → media envelope with the cleaned `.dcm` bytes.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker (no
//! ffmpeg / external dep). Surfaces: chat + CLI. No standalone page — binary
//! file-in / file-out has no page render mode (the no-page pattern, like
//! strip-exif / image-collage).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024; // 24 MiB
const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

fn default_profile() -> String {
    "basic".to_string()
}
fn default_placeholder() -> String {
    "ANON".to_string()
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_profile")]
    profile: String,
    #[serde(default = "default_placeholder")]
    placeholder: String,
}

/// Single-source param descriptor → chat schema (and CLI). The file input is
/// `Input::File` (url⊕ref); `profile` and `placeholder` are the redaction knobs.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("profile", ["basic", "strict"])
                .default("basic")
                .describe(
                    "How aggressively to redact. \"basic\" wipes the common patient-identifying \
                     tags (name, ID, birth date, sex, age, address, phone, referring/operator \
                     names, institution, accession number, study/series descriptions). \"strict\" \
                     also wipes every private (vendor-defined, odd-group) data element. Default \
                     \"basic\".",
                ),
        )
        .param(
            Param::string("placeholder").default("ANON").describe(
                "Text written into redacted string fields (space-padded to the original length so \
                 no byte offsets change). Binary/numeric fields are always zeroed. Default \"ANON\".",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DicomAnonymizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dicom-anonymizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove patient-identifying (PHI) data from a DICOM file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Anonymize a DICOM (.dcm) medical-imaging file by overwriting patient-identifying (PHI) data elements — PatientName, PatientID, birth date, sex, age, address, phone, referring/operator physician names, institution, accession number, study/series descriptions, and more — while preserving the pixel data and file structure byte-for-byte (redacted values keep their exact length, so nothing is re-encoded). Supports Explicit and Implicit VR Little Endian DICOM. Set profile=strict to also wipe private (vendor-defined) tags. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). Returns a sanitized .dcm file. Not a certified de-identification per DICOM PS3.15 Annex E — it covers the common direct identifiers, not burned-in pixel text.",
        parameters = schema_json()
    ),
)]
impl DicomAnonymizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("dicom-anonymizer")?;
    let profile = gizza_ai_dicom_anonymizer_core::Profile::parse(&args.profile)
        .map_err(SkillError::InvalidArgs)?;

    // 2. Resolve source — URL fetch or attachment lookup. DICOM is served under
    //    a variety of MIME types (application/dicom, octet-stream, …); we accept
    //    any bytes and validate the DICM magic ourselves in the core.
    let (input_bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;

    // 3. Anonymize (shared pure core).
    let (output, report) =
        gizza_ai_dicom_anonymizer_core::anonymize(&input_bytes, profile, &args.placeholder)
            .map_err(SkillError::InvalidArgs)?;

    // 4. Envelope — same DICOM bytes, PHI wiped, `-anonymized.dcm` name.
    let filename = filename_with_suffix(&in_filename, "-anonymized", "dcm");
    let for_llm = summary(&in_filename, &report);
    build_media_envelope(&output, "application/dicom", filename, for_llm, MAX_OUTPUT_BYTES)
}

/// One-line summary for the LLM: what was redacted and how much was preserved.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn summary(source: &str, report: &gizza_ai_dicom_anonymizer_core::Report) -> String {
    format!(
        "anonymized {} ({}): redacted {} PHI element(s) ({} bytes overwritten), {} of {} bytes \
         preserved; pixel data and structure unchanged",
        source,
        report.encoding,
        report.elements_redacted,
        report.bytes_redacted,
        report.bytes_preserved,
        report.total_bytes,
    )
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "profile": {
                        "type": "string",
                        "enum": ["basic", "strict"],
                        "default": "basic",
                        "description": "How aggressively to redact. \"basic\" wipes the common patient-identifying tags (name, ID, birth date, sex, age, address, phone, referring/operator names, institution, accession number, study/series descriptions). \"strict\" also wipes every private (vendor-defined, odd-group) data element. Default \"basic\"."
                    },
                    "placeholder": {
                        "type": "string",
                        "default": "ANON",
                        "description": "Text written into redacted string fields (space-padded to the original length so no byte offsets change). Binary/numeric fields are always zeroed. Default \"ANON\"."
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
    fn summary_reports_counts_and_preservation() {
        let report = gizza_ai_dicom_anonymizer_core::Report {
            elements_redacted: 3,
            bytes_redacted: 40,
            bytes_preserved: 960,
            total_bytes: 1000,
            encoding: "Explicit VR Little Endian",
        };
        let s = summary("scan.dcm", &report);
        assert!(s.contains("scan.dcm"));
        assert!(s.contains("3 PHI element"));
        assert!(s.contains("960 of 1000"));
        assert!(s.contains("pixel data and structure unchanged"));
    }

    #[test]
    fn anonymized_filename_keeps_dcm_extension() {
        assert_eq!(filename_with_suffix("scan.dcm", "-anonymized", "dcm"), "scan-anonymized.dcm");
        assert_eq!(filename_with_suffix("ct", "-anonymized", "dcm"), "ct-anonymized.dcm");
    }
}

//! gizza-ai/pe-info — parse a Windows PE/COFF binary (`.exe`, `.dll`, `.sys`)
//! and report its headers, sections, imports, and exports.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::parse` (pure, dependency-free PE parser) → flat JSON the LLM reads
//! directly (format, machine, kind, subsystem, image base, entry point,
//! sections, imported libraries + symbols, exported symbols).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→JSON report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the no-page file-input
//! pattern, like detect-file-type / web-fetch / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{AssetKind, Input, SkillError, SourceFields, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

// PE images can be large, but only the headers + directories matter; allow a
// generous cap so real binaries fit.
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct SectionResp {
    name: String,
    virtual_address: u32,
    virtual_size: u32,
    raw_size: u32,
    flags: Vec<String>,
}

#[derive(Serialize)]
struct ImportResp {
    library: String,
    function_count: usize,
    functions: Vec<String>,
}

#[derive(Serialize)]
struct Resp {
    /// `PE32` (32-bit) or `PE32+` (64-bit).
    format: String,
    /// Target machine architecture, e.g. `x86-64`.
    machine: String,
    /// `DLL`, `Driver`, or `Executable`.
    kind: String,
    is_dll: bool,
    /// Windows subsystem, e.g. `Windows GUI`.
    subsystem: String,
    /// Link timestamp as an ISO-8601 UTC string, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp_utc: Option<String>,
    /// Raw PE/COFF TimeDateStamp (seconds since the Unix epoch).
    timestamp: u32,
    /// Security/loader mitigation flags that are enabled.
    dll_characteristics: Vec<String>,
    image_base: String,
    entry_point: String,
    size_of_image: u32,
    section_count: u16,
    sections: Vec<SectionResp>,
    /// Internal DLL name from the export directory (for DLLs that export).
    #[serde(skip_serializing_if = "Option::is_none")]
    export_dll_name: Option<String>,
    import_count: usize,
    imports: Vec<ImportResp>,
    export_count: usize,
    exports: Vec<String>,
    bytes: usize,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a file arrives via URL fetch or
/// an attachment ref. No other parameters: parsing is fully automatic.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Format a Unix timestamp (seconds) as an ISO-8601 UTC string without pulling
/// in chrono. Returns None for 0 / out-of-range values.
fn iso_utc(secs: u32) -> Option<String> {
    if secs == 0 {
        return None;
    }
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z"
    ))
}

#[cfg(target_arch = "wasm32")]
struct PeInfoTool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pe-info",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a Windows PE/EXE/DLL and report its headers, sections, imports, and exports",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse a Windows Portable Executable (PE) binary — .exe, .dll, .sys, or other PE/COFF files — and report a structured summary of its internals. Returns the PE format (PE32 vs PE32+/64-bit), target machine architecture (x86, x86-64, ARM64, …), file kind (Executable/DLL/Driver), Windows subsystem (GUI/Console/Native/EFI), link timestamp, preferred image base and entry-point addresses, image size, enabled loader/security mitigation flags (ASLR, DEP/NX, Control Flow Guard, …), the section table (name, virtual/raw sizes, and access flags), the imported libraries with the symbols pulled from each, and the names a DLL exports. Useful for malware triage, reverse engineering, dependency inspection, and verifying build settings. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PeInfoTool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::SkillResultExt;
    let args: Args = serde_json::from_slice(&body).invalid_args("pe-info")?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let pe = gizza_ai_pe_info_core::parse(&bytes).map_err(SkillError::InvalidArgs)?;

    let resp = build_resp(&pe, bytes.len());
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pe-info response: {e}")))
}

/// Map the core parse result into the flat JSON response (shared by tests).
fn build_resp(pe: &gizza_ai_pe_info_core::PeInfo, byte_len: usize) -> Resp {
    Resp {
        format: pe.format.to_string(),
        machine: pe.machine.to_string(),
        kind: pe.kind.to_string(),
        is_dll: pe.is_dll,
        subsystem: pe.subsystem.to_string(),
        timestamp_utc: iso_utc(pe.timestamp),
        timestamp: pe.timestamp,
        dll_characteristics: pe
            .dll_characteristics
            .iter()
            .map(|s| s.to_string())
            .collect(),
        image_base: format!("0x{:x}", pe.image_base),
        entry_point: format!("0x{:x}", pe.entry_point),
        size_of_image: pe.size_of_image,
        section_count: pe.section_count,
        sections: pe
            .sections
            .iter()
            .map(|s| SectionResp {
                name: s.name.clone(),
                virtual_address: s.virtual_address,
                virtual_size: s.virtual_size,
                raw_size: s.raw_size,
                flags: s.flags.iter().map(|f| f.to_string()).collect(),
            })
            .collect(),
        export_dll_name: pe.export_dll_name.clone(),
        import_count: pe.imports.len(),
        imports: pe
            .imports
            .iter()
            .map(|l| ImportResp {
                library: l.name.clone(),
                function_count: l.functions.len(),
                functions: l.functions.clone(),
            })
            .collect(),
        export_count: pe.exports.len(),
        exports: pe.exports.clone(),
        bytes: byte_len,
    }
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
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
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
    fn iso_utc_formats_known_epoch() {
        // 2021-01-01T00:00:00Z = 1609459200.
        assert_eq!(iso_utc(1_609_459_200).as_deref(), Some("2021-01-01T00:00:00Z"));
        // The Unix epoch itself.
        assert_eq!(iso_utc(0), None);
        assert_eq!(iso_utc(1).as_deref(), Some("1970-01-01T00:00:01Z"));
    }

    #[test]
    fn build_resp_maps_core_fields() {
        let pe = gizza_ai_pe_info_core::PeInfo {
            format: "PE32+",
            machine: "x86-64",
            is_dll: true,
            kind: "DLL",
            subsystem: "Windows GUI",
            timestamp: 1_609_459_200,
            section_count: 2,
            image_base: 0x1_8000_0000,
            size_of_image: 0x10000,
            entry_point: 0x2000,
            dll_characteristics: vec!["dynamic base (ASLR)"],
            sections: vec![gizza_ai_pe_info_core::Section {
                name: ".text".into(),
                virtual_address: 0x1000,
                virtual_size: 0x500,
                raw_size: 0x600,
                flags: vec!["code", "executable"],
            }],
            imports: vec![gizza_ai_pe_info_core::ImportLib {
                name: "KERNEL32.dll".into(),
                functions: vec!["ExitProcess".into()],
            }],
            exports: vec!["DllMain".into()],
            export_dll_name: Some("sample.dll".into()),
        };
        let r = build_resp(&pe, 4096);
        assert_eq!(r.format, "PE32+");
        assert!(r.is_dll);
        assert_eq!(r.kind, "DLL");
        assert_eq!(r.image_base, "0x180000000");
        assert_eq!(r.entry_point, "0x2000");
        assert_eq!(r.timestamp_utc.as_deref(), Some("2021-01-01T00:00:00Z"));
        assert_eq!(r.import_count, 1);
        assert_eq!(r.imports[0].library, "KERNEL32.dll");
        assert_eq!(r.imports[0].function_count, 1);
        assert_eq!(r.export_count, 1);
        assert_eq!(r.exports, vec!["DllMain".to_string()]);
        assert_eq!(r.export_dll_name.as_deref(), Some("sample.dll"));
        assert_eq!(r.bytes, 4096);
    }
}

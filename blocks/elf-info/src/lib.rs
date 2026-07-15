//! gizza-ai/elf-info — parse an ELF binary (Linux/Unix executable, shared
//! object, or relocatable object) and report its header, architecture,
//! sections, segments, and symbol table.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::parse` (pure, dependency-free ELF parser) → flat JSON the LLM reads
//! directly (class, endianness, OS/ABI, file type, machine, entry point,
//! interpreter, sections, segments, symbols).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→JSON report fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the no-page file-input
//! pattern, like pe-info / detect-file-type / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{AssetKind, Input, SkillError, SourceFields, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

// ELF binaries can be large, but only the headers + tables matter; allow a
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
    #[serde(rename = "type")]
    sh_type: String,
    flags: Vec<String>,
    address: String,
    offset: u64,
    size: u64,
}

#[derive(Serialize)]
struct SegmentResp {
    #[serde(rename = "type")]
    p_type: String,
    flags: Vec<String>,
    offset: u64,
    virtual_address: String,
    file_size: u64,
    memory_size: u64,
}

#[derive(Serialize)]
struct SymbolResp {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    binding: String,
    value: String,
    size: u64,
    table: String,
}

#[derive(Serialize)]
struct Resp {
    /// `ELF32` or `ELF64`.
    class: String,
    /// `little-endian` or `big-endian`.
    endianness: String,
    version: u8,
    /// Target OS/ABI, e.g. `System V`, `Linux (GNU)`.
    os_abi: String,
    abi_version: u8,
    /// File type, e.g. `Executable`, `Shared object`, `Relocatable`, `Core`.
    file_type: String,
    /// Target machine architecture, e.g. `x86-64`, `AArch64 (ARM64)`.
    machine: String,
    /// True for ET_DYN (PIE executable or shared object).
    is_pie_or_shared: bool,
    /// Whether a PT_DYNAMIC segment is present (dynamically linked).
    is_dynamic: bool,
    entry_point: String,
    /// Requested program interpreter / dynamic linker (PT_INTERP), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    interpreter: Option<String>,
    /// Architecture-specific header flags that are set.
    flags: Vec<String>,
    program_header_count: u16,
    section_header_count: u16,
    sections: Vec<SectionResp>,
    segments: Vec<SegmentResp>,
    symbol_count: usize,
    symbols_truncated: bool,
    symbols: Vec<SymbolResp>,
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

#[cfg(target_arch = "wasm32")]
struct ElfInfoTool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/elf-info",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an ELF binary and report its header, architecture, sections, segments, and symbol table",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse an ELF (Executable and Linkable Format) binary — a Linux/Unix executable, shared object (.so), relocatable object (.o), or core dump — and report a structured summary of its internals. Returns the ELF class (32- vs 64-bit), byte order (little- or big-endian), OS/ABI, file type (Executable/Shared object/Relocatable/Core), target machine architecture (x86-64, AArch64/ARM64, RISC-V, MIPS, …), entry-point address, the requested program interpreter / dynamic linker (PT_INTERP), whether it is dynamically linked and position-independent (PIE), architecture-specific header flags, the section header table (name, type, flags, address, offset, size), the program header / segment table (LOAD/DYNAMIC/INTERP/GNU_STACK with read/write/execute permissions), and the symbol table from .symtab and .dynsym (name, type FUNC/OBJECT/…, binding LOCAL/GLOBAL/WEAK, address, size). Useful for reverse engineering, malware triage, dependency inspection, and verifying build settings. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ElfInfoTool {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("elf-info")?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let elf = gizza_ai_elf_info_core::parse(&bytes).map_err(SkillError::InvalidArgs)?;

    let resp = build_resp(&elf, bytes.len());
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize elf-info response: {e}")))
}

/// Map the core parse result into the flat JSON response (shared by tests).
fn build_resp(elf: &gizza_ai_elf_info_core::ElfInfo, byte_len: usize) -> Resp {
    Resp {
        class: elf.class.to_string(),
        endianness: elf.endianness.to_string(),
        version: elf.version,
        os_abi: elf.os_abi.to_string(),
        abi_version: elf.abi_version,
        file_type: elf.file_type.to_string(),
        machine: elf.machine.clone(),
        is_pie_or_shared: elf.is_pie_or_shared,
        is_dynamic: elf.is_dynamic,
        entry_point: format!("0x{:x}", elf.entry_point),
        interpreter: elf.interpreter.clone(),
        flags: elf.flags.clone(),
        program_header_count: elf.program_header_count,
        section_header_count: elf.section_header_count,
        sections: elf
            .sections
            .iter()
            .map(|s| SectionResp {
                name: s.name.clone(),
                sh_type: s.sh_type.clone(),
                flags: s.flags.iter().map(|f| f.to_string()).collect(),
                address: format!("0x{:x}", s.address),
                offset: s.offset,
                size: s.size,
            })
            .collect(),
        segments: elf
            .segments
            .iter()
            .map(|s| SegmentResp {
                p_type: s.p_type.clone(),
                flags: s.flags.iter().map(|f| f.to_string()).collect(),
                offset: s.offset,
                virtual_address: format!("0x{:x}", s.virtual_address),
                file_size: s.file_size,
                memory_size: s.memory_size,
            })
            .collect(),
        symbol_count: elf.symbols.len(),
        symbols_truncated: elf.symbols_truncated,
        symbols: elf
            .symbols
            .iter()
            .map(|s| SymbolResp {
                name: s.name.clone(),
                kind: s.kind.to_string(),
                binding: s.binding.to_string(),
                value: format!("0x{:x}", s.value),
                size: s.size,
                table: s.table.to_string(),
            })
            .collect(),
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
    fn build_resp_maps_core_fields() {
        let elf = gizza_ai_elf_info_core::ElfInfo {
            class: "ELF64",
            endianness: "little-endian",
            version: 1,
            os_abi: "System V",
            abi_version: 0,
            file_type: "Shared object",
            machine: "x86-64".to_string(),
            is_pie_or_shared: true,
            entry_point: 0x1040,
            program_header_count: 9,
            section_header_count: 29,
            flags: vec![],
            interpreter: Some("/lib64/ld-linux-x86-64.so.2".to_string()),
            is_dynamic: true,
            sections: vec![gizza_ai_elf_info_core::Section {
                name: ".text".into(),
                sh_type: "PROGBITS".into(),
                flags: vec!["alloc", "execinstr"],
                address: 0x1040,
                offset: 0x1040,
                size: 0x200,
            }],
            segments: vec![gizza_ai_elf_info_core::Segment {
                p_type: "LOAD".into(),
                flags: vec!["read", "execute"],
                offset: 0,
                virtual_address: 0,
                file_size: 0x600,
                memory_size: 0x600,
            }],
            symbols: vec![gizza_ai_elf_info_core::Symbol {
                name: "main".into(),
                kind: "FUNC",
                binding: "GLOBAL",
                value: 0x1149,
                size: 0x20,
                table: ".symtab",
            }],
            symbols_truncated: false,
        };
        let r = build_resp(&elf, 8192);
        assert_eq!(r.class, "ELF64");
        assert_eq!(r.machine, "x86-64");
        assert!(r.is_pie_or_shared);
        assert!(r.is_dynamic);
        assert_eq!(r.entry_point, "0x1040");
        assert_eq!(r.interpreter.as_deref(), Some("/lib64/ld-linux-x86-64.so.2"));
        assert_eq!(r.sections.len(), 1);
        assert_eq!(r.sections[0].name, ".text");
        assert_eq!(r.sections[0].address, "0x1040");
        assert_eq!(r.segments.len(), 1);
        assert_eq!(r.segments[0].p_type, "LOAD");
        assert_eq!(r.symbol_count, 1);
        assert_eq!(r.symbols[0].name, "main");
        assert_eq!(r.symbols[0].kind, "FUNC");
        assert_eq!(r.symbols[0].value, "0x1149");
        assert_eq!(r.bytes, 8192);
    }
}

//! gizza-ai/elf-macho-info — parse an ELF **or** Mach-O binary (Linux/BSD
//! executables and `.so` shared objects; macOS/iOS executables, `.dylib`
//! libraries, bundles, kexts, and fat/universal archives) and report one
//! unified summary.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::parse` (pure, dependency-free ELF + Mach-O parser) → a deterministic
//! plain-text report plus the same figures as flat JSON the LLM reads directly
//! (architecture, file type, flags, libraries, sections, symbols,
//! imports/exports, universal-binary slices).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→report tool fits neither the pure-text
//! page nor the ffmpeg file→media page shape — the no-page file-input pattern,
//! like elf-info / pe-info / detect-file-type / hex-view).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_elf_macho_info_core::{BinaryInfo, Capped, Options, MAX_LIMIT};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// ELF/Mach-O images can be large, but only the headers + tables matter; allow a
/// generous cap so real binaries (and fat archives, which are N binaries in one
/// file) fit without letting a hostile URL exhaust memory.
const MAX_BYTES: usize = 64 * 1024 * 1024;

const DEFAULT_LIMIT: u64 = 100;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "yes")]
    sections: bool,
    #[serde(default = "yes")]
    symbols: bool,
    #[serde(default = "yes")]
    imports: bool,
    #[serde(default = "default_limit")]
    limit: u64,
    #[serde(default)]
    arch: Option<String>,
}

fn yes() -> bool {
    true
}
fn default_limit() -> u64 {
    DEFAULT_LIMIT
}

impl Args {
    /// Map the wire arguments onto the core's [`Options`]. `limit` is clamped
    /// here as well as in the core so the response echo matches what was used,
    /// and a blank `arch` string is treated as "not supplied".
    fn to_options(&self) -> Options {
        Options {
            sections: self.sections,
            symbols: self.symbols,
            imports: self.imports,
            limit: (self.limit as usize).clamp(1, MAX_LIMIT),
            arch: self
                .arch
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        }
    }
}

#[derive(Serialize)]
struct SectionResp {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    flags: Vec<String>,
    address: String,
    offset: u64,
    size: u64,
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
struct LibraryResp {
    name: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility_version: Option<String>,
}

#[derive(Serialize)]
struct SliceResp {
    architecture: String,
    offset: u64,
    size: u64,
}

/// A capped list plus its pre-cap total, so the caller can say "showing 100 of
/// 41233" instead of silently truncating.
#[derive(Serialize)]
struct CappedResp<T> {
    items: Vec<T>,
    total: usize,
    truncated: bool,
}

impl<T> CappedResp<T> {
    fn map<U>(c: &Capped<U>, f: impl Fn(&U) -> T) -> Self {
        CappedResp {
            items: c.items.iter().map(f).collect(),
            total: c.total,
            truncated: c.truncated,
        }
    }
}

#[derive(Serialize)]
struct Resp {
    /// The whole summary as a deterministic plain-text report.
    report: String,
    /// `ELF` or `Mach-O`.
    format: String,
    /// 32 or 64.
    bits: u8,
    /// `little-endian` or `big-endian`.
    endianness: String,
    /// Target architecture, e.g. `x86-64`, `AArch64 (ARM64)`, `arm64e`.
    architecture: String,
    /// e.g. `Executable`, `Shared object / PIE`, `Dynamic library (dylib)`.
    file_type: String,
    /// ELF `EI_OSABI`. Absent for Mach-O.
    #[serde(skip_serializing_if = "Option::is_none")]
    os_abi: Option<String>,
    /// Mach-O deployment target, e.g. `macOS 14.0.0 (SDK 15.0.0)`. Absent for ELF.
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_point: Option<String>,
    is_pie: bool,
    is_dynamic: bool,
    is_stripped: bool,
    /// ELF `PT_INTERP`, or Mach-O `LC_LOAD_DYLINKER`.
    #[serde(skip_serializing_if = "Option::is_none")]
    linker: Option<String>,
    /// ELF `DT_SONAME`, or Mach-O `LC_ID_DYLIB` install name.
    #[serde(skip_serializing_if = "Option::is_none")]
    install_name: Option<String>,
    /// Mach-O `LC_UUID`.
    #[serde(skip_serializing_if = "Option::is_none")]
    uuid: Option<String>,
    /// Decoded header flags.
    flags: Vec<String>,
    /// ELF `DT_RPATH`/`DT_RUNPATH`, or Mach-O `LC_RPATH`.
    rpaths: Vec<String>,
    libraries: CappedResp<LibraryResp>,
    sections: CappedResp<SectionResp>,
    symbols: CappedResp<SymbolResp>,
    imports: CappedResp<String>,
    exports: CappedResp<String>,
    /// Slices of a fat/universal Mach-O; empty for a thin binary or an ELF.
    architectures: Vec<SliceResp>,
    /// Which fat slice this report describes.
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_architecture: Option<String>,
    bytes: usize,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a file arrives via URL fetch or
/// an attachment ref. The rest trim the report: the three list toggles and the
/// per-list `limit` keep a real binary's 40k-row symbol table out of the
/// response, and `arch` picks a slice out of a universal Mach-O.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::boolean("sections")
                .default(true)
                .describe("Include the section table (default true)."),
        )
        .param(
            Param::boolean("symbols")
                .default(true)
                .describe("Include the symbol table(s) (default true). Turn off for a fast header-only summary of a large binary."),
        )
        .param(
            Param::boolean("imports")
                .default(true)
                .describe("Include linked libraries, imported symbols, and exported symbols (default true)."),
        )
        .param(
            Param::integer("limit")
                .min(1.0)
                .max(MAX_LIMIT as f64)
                .default(DEFAULT_LIMIT)
                .describe("Maximum entries per list — sections, symbols, imports, exports (1-5000, default 100). Each list also reports its pre-cap total."),
        )
        .param(
            Param::string("arch")
                .describe("For a fat/universal Mach-O: which architecture slice to report, e.g. x86_64 or arm64. Defaults to the first slice."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ElfMachoInfoTool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/elf-macho-info",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an ELF or Mach-O binary and report its architecture, sections, symbols, and linked libraries",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse an ELF or Mach-O binary and report a structured summary of its internals. Handles ELF (Linux/BSD/Solaris executables, .so shared objects, .o relocatables, core dumps) and Mach-O (macOS/iOS executables, .dylib libraries, bundles, .kext kernel extensions), including fat/universal Mach-O archives that bundle several architecture slices — pass arch (e.g. x86_64, arm64) to pick one, otherwise the first slice is reported. Both formats come back in ONE shape: container format and word size (32/64-bit), byte order, target architecture, file type (Executable / Shared object / PIE / dylib / Bundle / kext / …), ELF OS-ABI or Mach-O deployment target, entry point, whether it is position-independent, dynamically linked, and stripped, the dynamic linker / interpreter, SONAME or install name, Mach-O LC_UUID, decoded header flags, RPATH/RUNPATH search paths, linked libraries (with Mach-O current/compatibility versions), the section table (name, type, flags, address, offset, size), the symbol table(s) from .symtab/.dynsym/LC_SYMTAB (name, type, binding, address, size), and the imported (undefined) and exported symbol names. Set sections, symbols, or imports to false to omit a list, and limit (1-5000, default 100) to cap entries per list — each list still reports its pre-cap total. Returns a plain-text report plus the same figures as JSON. Useful for reverse engineering, malware triage, dependency inspection, and verifying build settings. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ElfMachoInfoTool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("elf-macho-info")?;
    let opts = args.to_options();

    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let info =
        gizza_ai_elf_macho_info_core::parse(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let resp = build_resp(&info, &opts, bytes.len());
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize elf-macho-info response: {e}")))
}

/// Map the core parse result into the flat JSON response (shared by tests).
fn build_resp(info: &BinaryInfo, opts: &Options, byte_len: usize) -> Resp {
    Resp {
        report: render_report(info, opts, byte_len),
        format: info.format.to_string(),
        bits: info.bits,
        endianness: info.endianness.to_string(),
        architecture: info.architecture.clone(),
        file_type: info.file_type.clone(),
        os_abi: info.os_abi.clone(),
        platform: info.platform.clone(),
        entry_point: info.entry_point.map(|e| format!("0x{e:x}")),
        is_pie: info.is_pie,
        is_dynamic: info.is_dynamic,
        is_stripped: info.is_stripped,
        linker: info.linker.clone(),
        install_name: info.install_name.clone(),
        uuid: info.uuid.clone(),
        flags: info.flags.clone(),
        rpaths: info.rpaths.clone(),
        libraries: CappedResp::map(&info.libraries, |l| LibraryResp {
            name: l.name.clone(),
            kind: l.kind.to_string(),
            current_version: l.current_version.clone(),
            compatibility_version: l.compatibility_version.clone(),
        }),
        sections: CappedResp::map(&info.sections, |s| SectionResp {
            name: s.name.clone(),
            kind: s.kind.clone(),
            flags: s.flags.clone(),
            address: format!("0x{:x}", s.address),
            offset: s.offset,
            size: s.size,
        }),
        symbols: CappedResp::map(&info.symbols, |s| SymbolResp {
            name: s.name.clone(),
            kind: s.kind.clone(),
            binding: s.binding.clone(),
            value: format!("0x{:x}", s.value),
            size: s.size,
            table: s.table.clone(),
        }),
        imports: CappedResp::map(&info.imports, Clone::clone),
        exports: CappedResp::map(&info.exports, Clone::clone),
        architectures: info
            .architectures
            .iter()
            .map(|s| SliceResp {
                architecture: s.architecture.clone(),
                offset: s.offset,
                size: s.size,
            })
            .collect(),
        selected_architecture: info.selected_architecture.clone(),
        bytes: byte_len,
    }
}

// ---------------------------------------------------------------------------
// Text report
// ---------------------------------------------------------------------------

/// `Sections (6)` or, once the cap bit, `Sections (showing 100 of 41233)` — a
/// truncated list must never read like a complete one.
fn heading<T>(label: &str, c: &Capped<T>) -> String {
    if c.truncated {
        format!("{label} (showing {} of {}):", c.items.len(), c.total)
    } else {
        format!("{label} ({}):", c.total)
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Render the whole summary as deterministic plain text: the same input always
/// produces byte-identical output, and a list the caller switched off is
/// omitted entirely rather than shown as an ambiguous empty section.
fn render_report(info: &BinaryInfo, opts: &Options, byte_len: usize) -> String {
    let mut out = String::new();
    // A macro, not a closure: the blank-line separators between blocks touch
    // `out` directly, which a closure holding a `&mut out` would forbid.
    macro_rules! line {
        ($s:expr) => {{
            out.push_str(&$s);
            out.push('\n');
        }};
    }

    line!(format!(
        "{} {}-bit {}",
        info.format, info.bits, info.endianness
    ));
    line!(format!("Architecture: {}", info.architecture));
    line!(format!("File type: {}", info.file_type));
    if let Some(abi) = &info.os_abi {
        line!(format!("OS/ABI: {abi}"));
    }
    if let Some(p) = &info.platform {
        line!(format!("Platform: {p}"));
    }
    if let Some(e) = info.entry_point {
        line!(format!("Entry point: 0x{e:x}"));
    }
    line!(format!("Position-independent: {}", yes_no(info.is_pie)));
    line!(format!("Dynamically linked: {}", yes_no(info.is_dynamic)));
    line!(format!("Stripped: {}", yes_no(info.is_stripped)));
    if let Some(l) = &info.linker {
        line!(format!("Dynamic linker: {l}"));
    }
    if let Some(n) = &info.install_name {
        line!(format!("Install name / SONAME: {n}"));
    }
    if let Some(u) = &info.uuid {
        line!(format!("UUID: {u}"));
    }
    if !info.flags.is_empty() {
        line!(format!("Flags: {}", info.flags.join(", ")));
    }
    if !info.rpaths.is_empty() {
        line!(format!("Run paths: {}", info.rpaths.join(", ")));
    }
    line!(format!("File size: {byte_len} bytes"));

    if !info.architectures.is_empty() {
        out.push('\n');
        line!(format!(
            "Universal binary architectures ({}):",
            info.architectures.len()
        ));
        for s in &info.architectures {
            let selected = info.selected_architecture.as_deref() == Some(s.architecture.as_str());
            line!(format!(
                "  {}{} offset={} size={}",
                s.architecture,
                if selected { " (reported)" } else { "" },
                s.offset,
                s.size
            ));
        }
    }

    if opts.imports {
        out.push('\n');
        line!(heading("Libraries", &info.libraries));
        if info.libraries.items.is_empty() {
            line!("  (none)".to_string());
        }
        for l in &info.libraries.items {
            let mut extra = String::new();
            if let Some(v) = &l.current_version {
                extra.push_str(&format!(" current={v}"));
            }
            if let Some(v) = &l.compatibility_version {
                extra.push_str(&format!(" compat={v}"));
            }
            line!(format!("  {} [{}]{}", l.name, l.kind, extra));
        }
    }

    if opts.sections {
        out.push('\n');
        line!(heading("Sections", &info.sections));
        if info.sections.items.is_empty() {
            line!("  (none)".to_string());
        }
        for s in &info.sections.items {
            let flags = if s.flags.is_empty() {
                "-".to_string()
            } else {
                s.flags.join(",")
            };
            line!(format!(
                "  {} type={} addr=0x{:x} offset={} size={} flags={}",
                s.name, s.kind, s.address, s.offset, s.size, flags
            ));
        }
    }

    if opts.symbols {
        out.push('\n');
        line!(heading("Symbols", &info.symbols));
        if info.symbols.items.is_empty() {
            line!("  (none)".to_string());
        }
        for s in &info.symbols.items {
            let name = if s.name.is_empty() {
                "(unnamed)"
            } else {
                s.name.as_str()
            };
            line!(format!(
                "  {} type={} binding={} value=0x{:x} size={} table={}",
                name, s.kind, s.binding, s.value, s.size, s.table
            ));
        }
    }

    if opts.imports {
        for (label, list) in [
            ("Imported symbols", &info.imports),
            ("Exported symbols", &info.exports),
        ] {
            out.push('\n');
            line!(heading(label, list));
            if list.items.is_empty() {
                line!("  (none)".to_string());
            }
            for n in &list.items {
                line!(format!("  {n}"));
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizza_ai_elf_macho_info_core as core;

    // -- schema drift guards ------------------------------------------------

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "sections": { "type": "boolean", "default": true, "description": "Include the section table (default true)." },
                    "symbols": { "type": "boolean", "default": true, "description": "Include the symbol table(s) (default true). Turn off for a fast header-only summary of a large binary." },
                    "imports": { "type": "boolean", "default": true, "description": "Include linked libraries, imported symbols, and exported symbols (default true)." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 100, "description": "Maximum entries per list — sections, symbols, imports, exports (1-5000, default 100). Each list also reports its pre-cap total." },
                    "arch": { "type": "string", "description": "For a fat/universal Mach-O: which architecture slice to report, e.g. x86_64 or arm64. Defaults to the first slice." }
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
    fn schema_bounds_track_the_core_cap() {
        // The advertised `limit` maximum must be the ceiling the core actually
        // clamps to, or the LLM sends values that are silently reduced.
        let d = descriptor();
        let p = d.params.iter().find(|p| p.name == "limit").expect("limit");
        assert_eq!(p.maximum, Some(core::MAX_LIMIT as f64));
        assert_eq!(p.minimum, Some(1.0));
        assert_eq!(p.default, Some(serde_json::json!(100)));
        assert!(!p.required, "limit is optional");
    }

    #[test]
    fn descriptor_declares_a_file_input_and_no_required_params() {
        let d = descriptor();
        assert_eq!(d.input, Input::File);
        assert!(
            d.params.iter().all(|p| !p.required),
            "the file itself is the only required input"
        );
        let names: Vec<&str> = d.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["sections", "symbols", "imports", "limit", "arch"]);
    }

    // -- argument mapping ---------------------------------------------------

    fn args_from(json: &str) -> Args {
        serde_json::from_str(json).expect("args parse")
    }

    #[test]
    fn args_default_to_every_list_on_and_limit_100() {
        let a = args_from(r#"{"url":"https://example.com/a.out"}"#);
        let o = a.to_options();
        assert_eq!(o, Options::default());
        assert!(o.sections && o.symbols && o.imports);
        assert_eq!(o.limit, 100);
        assert_eq!(o.arch, None);
    }

    #[test]
    fn args_clamp_limit_and_normalize_arch() {
        let a = args_from(r#"{"ref":"abc","limit":0,"arch":"  arm64 "}"#);
        let o = a.to_options();
        assert_eq!(o.limit, 1, "0 clamps up to 1, never down to an empty list");
        assert_eq!(o.arch.as_deref(), Some("arm64"));

        let a = args_from(r#"{"ref":"abc","limit":999999}"#);
        assert_eq!(a.to_options().limit, core::MAX_LIMIT);

        let a = args_from(r#"{"ref":"abc","arch":"   "}"#);
        assert_eq!(a.to_options().arch, None, "blank arch means 'first slice'");
    }

    #[test]
    fn args_honor_list_toggles() {
        let a = args_from(r#"{"ref":"abc","sections":false,"symbols":false,"imports":true}"#);
        let o = a.to_options();
        assert!(!o.sections && !o.symbols && o.imports);
    }

    // -- report rendering ---------------------------------------------------

    fn capped<T>(items: Vec<T>, total: usize) -> Capped<T> {
        Capped {
            truncated: total > items.len(),
            total,
            items,
        }
    }

    fn sample_elf() -> BinaryInfo {
        BinaryInfo {
            format: "ELF",
            bits: 64,
            endianness: "little-endian",
            architecture: "x86-64".to_string(),
            file_type: "Shared object / PIE".to_string(),
            os_abi: Some("System V".to_string()),
            platform: None,
            entry_point: Some(0x1040),
            is_pie: true,
            is_dynamic: true,
            is_stripped: false,
            linker: Some("/lib64/ld-linux-x86-64.so.2".to_string()),
            install_name: Some("libtest.so".to_string()),
            uuid: None,
            flags: vec![],
            rpaths: vec!["/opt/lib".to_string()],
            libraries: capped(
                vec![core::Library {
                    name: "libc.so.6".to_string(),
                    kind: "NEEDED",
                    current_version: None,
                    compatibility_version: None,
                }],
                1,
            ),
            sections: capped(
                vec![core::Section {
                    name: ".text".to_string(),
                    kind: "PROGBITS".to_string(),
                    flags: vec!["alloc".to_string(), "execinstr".to_string()],
                    address: 0x1040,
                    offset: 0x1040,
                    size: 0x200,
                }],
                29,
            ),
            symbols: capped(
                vec![core::Symbol {
                    name: "main".to_string(),
                    kind: "FUNC".to_string(),
                    binding: "GLOBAL".to_string(),
                    value: 0x1149,
                    size: 0x20,
                    table: ".symtab".to_string(),
                }],
                1,
            ),
            imports: capped(vec!["puts".to_string()], 1),
            exports: capped(vec!["my_func".to_string()], 1),
            architectures: vec![],
            selected_architecture: None,
        }
    }

    #[test]
    fn build_resp_maps_core_fields() {
        let info = sample_elf();
        let r = build_resp(&info, &Options::default(), 8192);
        assert_eq!(r.format, "ELF");
        assert_eq!(r.bits, 64);
        assert_eq!(r.architecture, "x86-64");
        assert_eq!(r.file_type, "Shared object / PIE");
        assert_eq!(r.os_abi.as_deref(), Some("System V"));
        assert_eq!(r.platform, None);
        assert_eq!(r.entry_point.as_deref(), Some("0x1040"));
        assert!(r.is_pie && r.is_dynamic && !r.is_stripped);
        assert_eq!(r.install_name.as_deref(), Some("libtest.so"));
        assert_eq!(r.rpaths, vec!["/opt/lib".to_string()]);
        assert_eq!(r.libraries.items[0].name, "libc.so.6");
        assert_eq!(r.libraries.items[0].kind, "NEEDED");
        assert_eq!(r.sections.items[0].address, "0x1040");
        assert_eq!(r.sections.total, 29);
        assert!(r.sections.truncated, "1 of 29 shown");
        assert_eq!(r.symbols.items[0].value, "0x1149");
        assert_eq!(r.imports.items, vec!["puts".to_string()]);
        assert_eq!(r.exports.items, vec!["my_func".to_string()]);
        assert!(r.architectures.is_empty());
        assert_eq!(r.bytes, 8192);
    }

    #[test]
    fn response_omits_absent_optional_fields() {
        let mut info = sample_elf();
        info.linker = None;
        info.install_name = None;
        info.entry_point = None;
        let v = serde_json::to_value(build_resp(&info, &Options::default(), 10)).unwrap();
        for absent in ["platform", "uuid", "linker", "install_name", "entry_point"] {
            assert!(v.get(absent).is_none(), "{absent} should be omitted");
        }
        assert!(v.get("os_abi").is_some(), "present fields still serialize");
    }

    #[test]
    fn report_is_deterministic_and_states_the_headline_facts() {
        let info = sample_elf();
        let a = render_report(&info, &Options::default(), 8192);
        let b = render_report(&info, &Options::default(), 8192);
        assert_eq!(a, b, "same input, byte-identical report");

        assert!(a.starts_with("ELF 64-bit little-endian\n"));
        assert!(a.contains("Architecture: x86-64\n"));
        assert!(a.contains("File type: Shared object / PIE\n"));
        assert!(a.contains("OS/ABI: System V\n"));
        assert!(a.contains("Entry point: 0x1040\n"));
        assert!(a.contains("Position-independent: yes\n"));
        assert!(a.contains("Dynamically linked: yes\n"));
        assert!(a.contains("Stripped: no\n"));
        assert!(a.contains("Dynamic linker: /lib64/ld-linux-x86-64.so.2\n"));
        assert!(a.contains("Install name / SONAME: libtest.so\n"));
        assert!(a.contains("Run paths: /opt/lib\n"));
        assert!(a.contains("File size: 8192 bytes\n"));
        assert!(a.contains("Libraries (1):\n  libc.so.6 [NEEDED]\n"));
        assert!(a.contains("  .text type=PROGBITS addr=0x1040 offset=4160 size=512 flags=alloc,execinstr\n"));
        assert!(a.contains(
            "  main type=FUNC binding=GLOBAL value=0x1149 size=32 table=.symtab\n"
        ));
        assert!(a.contains("Imported symbols (1):\n  puts\n"));
        assert!(a.contains("Exported symbols (1):\n  my_func\n"));
    }

    #[test]
    fn report_says_showing_n_of_m_when_a_list_is_capped() {
        let a = render_report(&sample_elf(), &Options::default(), 8192);
        assert!(
            a.contains("Sections (showing 1 of 29):"),
            "a truncated list must not read as complete: {a}"
        );
        // Untruncated lists keep the plain count.
        assert!(a.contains("Symbols (1):"));
    }

    #[test]
    fn report_omits_lists_the_caller_switched_off() {
        let opts = Options {
            sections: false,
            symbols: false,
            imports: false,
            ..Options::default()
        };
        // Mirrors what the core returns for disabled lists: empty, total 0.
        let mut info = sample_elf();
        info.sections = capped(vec![], 0);
        info.symbols = capped(vec![], 0);
        info.libraries = capped(vec![], 0);
        info.imports = capped(vec![], 0);
        info.exports = capped(vec![], 0);

        let a = render_report(&info, &opts, 8192);
        for absent in [
            "Sections",
            "Symbols",
            "Libraries",
            "Imported symbols",
            "Exported symbols",
        ] {
            assert!(!a.contains(absent), "{absent} should be omitted: {a}");
        }
        // The header block is always there.
        assert!(a.contains("Architecture: x86-64\n"));
    }

    #[test]
    fn report_marks_empty_lists_explicitly() {
        let mut info = sample_elf();
        info.libraries = capped(vec![], 0);
        let a = render_report(&info, &Options::default(), 8192);
        assert!(a.contains("Libraries (0):\n  (none)\n"));
    }

    #[test]
    fn report_lists_universal_slices_and_marks_the_reported_one() {
        let mut info = sample_elf();
        info.format = "Mach-O";
        info.os_abi = None;
        info.platform = Some("macOS 14.0.0 (SDK 15.0.0)".to_string());
        info.uuid = Some("00010203-0405-0607-0809-0a0b0c0d0e0f".to_string());
        info.flags = vec!["DYLDLINK".to_string(), "PIE".to_string()];
        info.architectures = vec![
            core::Slice {
                architecture: "x86_64".to_string(),
                offset: 16384,
                size: 4096,
            },
            core::Slice {
                architecture: "arm64".to_string(),
                offset: 32768,
                size: 4096,
            },
        ];
        info.selected_architecture = Some("arm64".to_string());

        let a = render_report(&info, &Options::default(), 65536);
        assert!(a.contains("Platform: macOS 14.0.0 (SDK 15.0.0)\n"));
        assert!(a.contains("UUID: 00010203-0405-0607-0809-0a0b0c0d0e0f\n"));
        assert!(a.contains("Flags: DYLDLINK, PIE\n"));
        assert!(a.contains("Universal binary architectures (2):\n"));
        assert!(a.contains("  x86_64 offset=16384 size=4096\n"));
        assert!(a.contains("  arm64 (reported) offset=32768 size=4096\n"));
        assert!(!a.contains("OS/ABI"), "Mach-O has no ELF OS/ABI line");
    }

    #[test]
    fn report_renders_macho_library_versions_and_unnamed_symbols() {
        let mut info = sample_elf();
        info.libraries = capped(
            vec![core::Library {
                name: "/usr/lib/libSystem.B.dylib".to_string(),
                kind: "LOAD_DYLIB",
                current_version: Some("1319.0.0".to_string()),
                compatibility_version: Some("1.0.0".to_string()),
            }],
            1,
        );
        info.symbols = capped(
            vec![core::Symbol {
                name: String::new(),
                kind: "NOTYPE".to_string(),
                binding: "LOCAL".to_string(),
                value: 0,
                size: 0,
                table: ".dynsym".to_string(),
            }],
            1,
        );
        let a = render_report(&info, &Options::default(), 4096);
        assert!(a.contains(
            "  /usr/lib/libSystem.B.dylib [LOAD_DYLIB] current=1319.0.0 compat=1.0.0\n"
        ));
        assert!(
            a.contains("  (unnamed) type=NOTYPE"),
            "a nameless symbol still renders a readable row: {a}"
        );
    }

    // -- end-to-end over the real parser ------------------------------------

    #[test]
    fn end_to_end_over_a_real_elf64_header() {
        // Smallest thing the core accepts as an ELF: just the 64-byte header.
        let mut b = vec![0u8; 64];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = 2; // 64-bit
        b[5] = 1; // little-endian
        b[6] = 1; // version
        b[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        b[18..20].copy_from_slice(&62u16.to_le_bytes()); // EM_X86_64
        b[24..32].copy_from_slice(&0x401000u64.to_le_bytes()); // e_entry

        let opts = Options::default();
        let info = core::parse(&b, &opts).expect("parses");
        let r = build_resp(&info, &opts, b.len());
        assert_eq!(r.format, "ELF");
        assert_eq!(r.architecture, "x86-64");
        assert_eq!(r.file_type, "Executable");
        assert_eq!(r.entry_point.as_deref(), Some("0x401000"));
        assert_eq!(r.bytes, 64);
        assert!(r.report.starts_with("ELF 64-bit little-endian\n"));
        assert!(r.report.contains("Sections (0):\n  (none)\n"));
    }
}

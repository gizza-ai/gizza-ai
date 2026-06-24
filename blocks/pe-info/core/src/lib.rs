//! gizza-ai/pe-info core — pure, dependency-free parser for the Windows PE
//! (Portable Executable) container used by `.exe`, `.dll`, `.sys` and other
//! Windows binaries.
//!
//! Parses the MS-DOS stub, the PE/COFF file header, the optional header
//! (PE32 / PE32+), the section table, and walks the import and export data
//! directories. No external crates → instantiates cleanly in the wafer
//! (wasm32-wasip1) runtime and runs on every backend including the chat
//! Service Worker.
//!
//! References: Microsoft "PE Format" spec (winnt.h structures).

/// Parsed summary of a PE binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeInfo {
    /// `PE32` (32-bit) or `PE32+` (64-bit), from the optional-header magic.
    pub format: &'static str,
    /// Target machine architecture, e.g. `x86`, `x86-64`, `ARM64`.
    pub machine: &'static str,
    /// Whether the COFF characteristics mark this as a DLL.
    pub is_dll: bool,
    /// Coarse kind: `DLL`, `Driver`, or `Executable`.
    pub kind: &'static str,
    /// Subsystem, e.g. `Windows GUI`, `Windows Console`, `Native`.
    pub subsystem: &'static str,
    /// PE/COFF `TimeDateStamp` (seconds since the Unix epoch), 0 when unset.
    pub timestamp: u32,
    /// Number of sections declared in the file header.
    pub section_count: u16,
    /// Preferred load address (`ImageBase`).
    pub image_base: u64,
    /// Total size of the loaded image in memory (`SizeOfImage`).
    pub size_of_image: u32,
    /// Entry-point RVA (`AddressOfEntryPoint`).
    pub entry_point: u32,
    /// Decoded DLL characteristics flags (ASLR / DEP / etc.) that are set.
    pub dll_characteristics: Vec<&'static str>,
    /// One entry per section.
    pub sections: Vec<Section>,
    /// Imported libraries with the symbols pulled from each.
    pub imports: Vec<ImportLib>,
    /// Exported symbol names (functions a DLL exposes).
    pub exports: Vec<String>,
    /// Internal DLL name from the export directory, if present.
    pub export_dll_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub raw_size: u32,
    /// Decoded section characteristics (e.g. `code`, `readable`, `writable`).
    pub flags: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportLib {
    pub name: String,
    pub functions: Vec<String>,
}

// Cap how much we list so a huge binary can't produce a megabyte of JSON.
const MAX_IMPORT_LIBS: usize = 256;
const MAX_FUNCS_PER_LIB: usize = 4096;
const MAX_EXPORTS: usize = 8192;

fn rd_u16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn rd_u64(b: &[u8], off: usize) -> Option<u64> {
    b.get(off..off + 8)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

fn machine_name(m: u16) -> &'static str {
    match m {
        0x014c => "x86",
        0x8664 => "x86-64",
        0x01c0 => "ARM",
        0x01c4 => "ARMv7 (Thumb-2)",
        0xaa64 => "ARM64",
        0x0200 => "Itanium (IA-64)",
        0x0ebc => "EFI byte code",
        0x5032 => "RISC-V 32",
        0x5064 => "RISC-V 64",
        0x0000 => "unknown (any)",
        _ => "unknown",
    }
}

fn subsystem_name(s: u16) -> &'static str {
    match s {
        0 => "unknown",
        1 => "Native",
        2 => "Windows GUI",
        3 => "Windows Console",
        5 => "OS/2 Console",
        7 => "POSIX Console",
        9 => "Windows CE GUI",
        10 => "EFI application",
        11 => "EFI boot service driver",
        12 => "EFI runtime driver",
        13 => "EFI ROM",
        14 => "Xbox",
        16 => "Windows boot application",
        _ => "unknown",
    }
}

fn dll_characteristics(flags: u16) -> Vec<&'static str> {
    let mut v = Vec::new();
    if flags & 0x0020 != 0 {
        v.push("high-entropy ASLR (64-bit)");
    }
    if flags & 0x0040 != 0 {
        v.push("dynamic base (ASLR)");
    }
    if flags & 0x0080 != 0 {
        v.push("force integrity check");
    }
    if flags & 0x0100 != 0 {
        v.push("NX compatible (DEP)");
    }
    if flags & 0x0200 != 0 {
        v.push("no isolation");
    }
    if flags & 0x0400 != 0 {
        v.push("no SEH");
    }
    if flags & 0x0800 != 0 {
        v.push("do not bind");
    }
    if flags & 0x1000 != 0 {
        v.push("AppContainer");
    }
    if flags & 0x2000 != 0 {
        v.push("WDM driver");
    }
    if flags & 0x4000 != 0 {
        v.push("Control Flow Guard");
    }
    if flags & 0x8000 != 0 {
        v.push("terminal-server aware");
    }
    v
}

fn section_flags(c: u32) -> Vec<&'static str> {
    let mut v = Vec::new();
    if c & 0x0000_0020 != 0 {
        v.push("code");
    }
    if c & 0x0000_0040 != 0 {
        v.push("initialized data");
    }
    if c & 0x0000_0080 != 0 {
        v.push("uninitialized data");
    }
    if c & 0x0200_0000 != 0 {
        v.push("discardable");
    }
    if c & 0x1000_0000 != 0 {
        v.push("shared");
    }
    if c & 0x2000_0000 != 0 {
        v.push("executable");
    }
    if c & 0x4000_0000 != 0 {
        v.push("readable");
    }
    if c & 0x8000_0000 != 0 {
        v.push("writable");
    }
    v
}

/// A loaded section, used to translate RVAs to file offsets.
struct SecMap {
    va: u32,
    vsize: u32,
    raw_ptr: u32,
    raw_size: u32,
}

/// Translate a relative virtual address to a file offset using the section map.
fn rva_to_off(secs: &[SecMap], rva: u32) -> Option<usize> {
    for s in secs {
        // Use the larger of virtual/raw size to bound membership.
        let span = s.vsize.max(s.raw_size);
        if rva >= s.va && rva < s.va.saturating_add(span) {
            let delta = rva - s.va;
            if delta < s.raw_size {
                return Some((s.raw_ptr + delta) as usize);
            }
        }
    }
    None
}

/// Read a NUL-terminated ASCII string at `off`, bounded by `max` bytes.
fn read_cstr(b: &[u8], off: usize, max: usize) -> Option<String> {
    let slice = b.get(off..)?;
    let end = slice.iter().take(max).position(|&c| c == 0)?;
    Some(String::from_utf8_lossy(&slice[..end]).into_owned())
}

/// Parse a PE binary from its raw bytes. Returns a descriptive error if the
/// bytes are not a valid PE image.
pub fn parse(b: &[u8]) -> Result<PeInfo, String> {
    if b.len() < 64 {
        return Err("file too small to be a PE binary".into());
    }
    if &b[0..2] != b"MZ" {
        return Err("not a PE file: missing the 'MZ' DOS signature".into());
    }
    // e_lfanew at 0x3C points to the PE header.
    let pe_off = rd_u32(b, 0x3c).ok_or("truncated DOS header (no e_lfanew)")? as usize;
    if b.get(pe_off..pe_off + 4) != Some(b"PE\0\0") {
        return Err("not a PE file: missing the 'PE\\0\\0' signature".into());
    }

    // COFF file header (20 bytes) immediately after the PE signature.
    let coff = pe_off + 4;
    let machine = rd_u16(b, coff).ok_or("truncated COFF header")?;
    let section_count = rd_u16(b, coff + 2).ok_or("truncated COFF header")?;
    let timestamp = rd_u32(b, coff + 4).unwrap_or(0);
    let opt_size = rd_u16(b, coff + 16).ok_or("truncated COFF header")? as usize;
    let characteristics = rd_u16(b, coff + 18).ok_or("truncated COFF header")?;
    let is_dll = characteristics & 0x2000 != 0;
    let is_sys = characteristics & 0x1000 != 0; // IMAGE_FILE_SYSTEM

    // Optional header.
    let opt = coff + 20;
    let magic = rd_u16(b, opt).ok_or("missing optional header")?;
    let (format, is_pe32_plus) = match magic {
        0x10b => ("PE32", false),
        0x20b => ("PE32+", true),
        0x107 => return Err("ROM image (optional-header magic 0x107) is not supported".into()),
        other => return Err(format!("unknown optional-header magic 0x{other:x}")),
    };

    let (image_base, after_base, entry_point) = if is_pe32_plus {
        let ep = rd_u32(b, opt + 16).ok_or("truncated optional header")?;
        let base = rd_u64(b, opt + 24).ok_or("truncated optional header")?;
        (base, opt + 24 + 8, ep)
    } else {
        let ep = rd_u32(b, opt + 16).ok_or("truncated optional header")?;
        let base = rd_u32(b, opt + 28).ok_or("truncated optional header")? as u64;
        (base, opt + 28 + 4, ep)
    };
    let _ = after_base;

    // Fields at fixed offsets within the optional header.
    let size_of_image = if is_pe32_plus {
        rd_u32(b, opt + 56)
    } else {
        rd_u32(b, opt + 56)
    }
    .unwrap_or(0);
    let subsystem = rd_u16(b, opt + 68).map(subsystem_name).unwrap_or("unknown");
    let dll_chars = rd_u16(b, opt + 70).map(dll_characteristics).unwrap_or_default();

    // Data directories: NumberOfRvaAndSizes at opt+92 (PE32) / opt+108 (PE32+).
    let (num_dirs_off, dirs_off) = if is_pe32_plus {
        (opt + 108, opt + 112)
    } else {
        (opt + 92, opt + 96)
    };
    let num_dirs = rd_u32(b, num_dirs_off).unwrap_or(0) as usize;
    // Directory 0 = export, 1 = import.
    let dir = |idx: usize| -> Option<(u32, u32)> {
        if idx >= num_dirs {
            return None;
        }
        let base = dirs_off + idx * 8;
        Some((rd_u32(b, base)?, rd_u32(b, base + 4)?))
    };
    let export_dir = dir(0);
    let import_dir = dir(1);

    // Section table follows the optional header.
    let sec_table = opt + opt_size;
    let mut sections = Vec::new();
    let mut secmap = Vec::new();
    for i in 0..section_count as usize {
        let so = sec_table + i * 40;
        let raw_name = match b.get(so..so + 8) {
            Some(n) => n,
            None => break,
        };
        let end = raw_name.iter().position(|&c| c == 0).unwrap_or(8);
        let name = String::from_utf8_lossy(&raw_name[..end]).into_owned();
        let virtual_size = rd_u32(b, so + 8).unwrap_or(0);
        let virtual_address = rd_u32(b, so + 12).unwrap_or(0);
        let raw_size = rd_u32(b, so + 16).unwrap_or(0);
        let raw_ptr = rd_u32(b, so + 20).unwrap_or(0);
        let chars = rd_u32(b, so + 36).unwrap_or(0);
        secmap.push(SecMap {
            va: virtual_address,
            vsize: virtual_size,
            raw_ptr,
            raw_size,
        });
        sections.push(Section {
            name,
            virtual_address,
            virtual_size,
            raw_size,
            flags: section_flags(chars),
        });
    }

    let imports = parse_imports(b, &secmap, import_dir, is_pe32_plus);
    let (exports, export_dll_name) = parse_exports(b, &secmap, export_dir);

    let kind = if is_sys {
        "Driver"
    } else if is_dll {
        "DLL"
    } else {
        "Executable"
    };

    Ok(PeInfo {
        format,
        machine: machine_name(machine),
        is_dll,
        kind,
        subsystem,
        timestamp,
        section_count,
        image_base,
        size_of_image,
        entry_point,
        dll_characteristics: dll_chars,
        sections,
        imports,
        exports,
        export_dll_name,
    })
}

fn parse_imports(
    b: &[u8],
    secs: &[SecMap],
    import_dir: Option<(u32, u32)>,
    is_pe32_plus: bool,
) -> Vec<ImportLib> {
    let mut libs = Vec::new();
    let (rva, _size) = match import_dir {
        Some((r, s)) if r != 0 => (r, s),
        _ => return libs,
    };
    let mut idt = match rva_to_off(secs, rva) {
        Some(o) => o,
        None => return libs,
    };
    // Each Import Directory Table entry is 20 bytes; terminated by all-zero.
    loop {
        if libs.len() >= MAX_IMPORT_LIBS {
            break;
        }
        let orig_first_thunk = match rd_u32(b, idt) {
            Some(v) => v,
            None => break,
        };
        let name_rva = rd_u32(b, idt + 12).unwrap_or(0);
        let first_thunk = rd_u32(b, idt + 16).unwrap_or(0);
        if orig_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
            break; // null terminator entry
        }
        let lib_name = name_rva
            .checked_sub(0)
            .and_then(|r| rva_to_off(secs, r))
            .and_then(|o| read_cstr(b, o, 256))
            .unwrap_or_else(|| "<unknown>".into());

        // Prefer the OriginalFirstThunk (import lookup table); fall back to the IAT.
        let thunk_rva = if orig_first_thunk != 0 {
            orig_first_thunk
        } else {
            first_thunk
        };
        let mut functions = Vec::new();
        if let Some(mut thunk_off) = rva_to_off(secs, thunk_rva) {
            let (step, ord_flag) = if is_pe32_plus {
                (8usize, 0x8000_0000_0000_0000u64)
            } else {
                (4usize, 0x8000_0000u64)
            };
            loop {
                if functions.len() >= MAX_FUNCS_PER_LIB {
                    break;
                }
                let entry = if is_pe32_plus {
                    match rd_u64(b, thunk_off) {
                        Some(v) => v,
                        None => break,
                    }
                } else {
                    match rd_u32(b, thunk_off) {
                        Some(v) => v as u64,
                        None => break,
                    }
                };
                if entry == 0 {
                    break;
                }
                if entry & ord_flag != 0 {
                    let ord = entry & 0xffff;
                    functions.push(format!("#{ord} (ordinal)"));
                } else {
                    // Low 31 bits = RVA of an IMAGE_IMPORT_BY_NAME (2-byte hint + name).
                    let hint_rva = (entry & 0x7fff_ffff) as u32;
                    if let Some(off) = rva_to_off(secs, hint_rva) {
                        if let Some(name) = read_cstr(b, off + 2, 512) {
                            functions.push(name);
                        }
                    }
                }
                thunk_off += step;
            }
        }
        libs.push(ImportLib {
            name: lib_name,
            functions,
        });
        idt += 20;
    }
    libs
}

fn parse_exports(
    b: &[u8],
    secs: &[SecMap],
    export_dir: Option<(u32, u32)>,
) -> (Vec<String>, Option<String>) {
    let mut names = Vec::new();
    let (rva, _size) = match export_dir {
        Some((r, s)) if r != 0 => (r, s),
        _ => return (names, None),
    };
    let edt = match rva_to_off(secs, rva) {
        Some(o) => o,
        None => return (names, None),
    };
    // IMAGE_EXPORT_DIRECTORY: Name at +12, NumberOfNames at +24,
    // AddressOfNames at +32.
    let name_rva = rd_u32(b, edt + 12).unwrap_or(0);
    let dll_name = rva_to_off(secs, name_rva).and_then(|o| read_cstr(b, o, 256));
    let num_names = rd_u32(b, edt + 24).unwrap_or(0) as usize;
    let names_rva = rd_u32(b, edt + 32).unwrap_or(0);

    if let Some(names_off) = rva_to_off(secs, names_rva) {
        for i in 0..num_names.min(MAX_EXPORTS) {
            let entry_off = names_off + i * 4;
            let str_rva = match rd_u32(b, entry_off) {
                Some(v) => v,
                None => break,
            };
            if let Some(o) = rva_to_off(secs, str_rva) {
                if let Some(name) = read_cstr(b, o, 512) {
                    names.push(name);
                }
            }
        }
    }
    (names, dll_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal but structurally valid PE32+ executable in memory:
    /// DOS header → PE sig → COFF header → optional header (with data
    /// directories) → one `.text` section → an import table referencing
    /// KERNEL32.dll!ExitProcess.
    fn build_pe() -> Vec<u8> {
        let mut b = vec![0u8; 0x600];
        // DOS header.
        b[0] = b'M';
        b[1] = b'Z';
        let pe_off: u32 = 0x80;
        b[0x3c..0x40].copy_from_slice(&pe_off.to_le_bytes());
        // PE signature.
        let po = pe_off as usize;
        b[po..po + 4].copy_from_slice(b"PE\0\0");
        // COFF header.
        let coff = po + 4;
        b[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes()); // x86-64
        b[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes()); // 1 section
        b[coff + 4..coff + 8].copy_from_slice(&0x6000_0000u32.to_le_bytes()); // timestamp
        let opt_size: u16 = 240;
        b[coff + 16..coff + 18].copy_from_slice(&opt_size.to_le_bytes());
        b[coff + 18..coff + 20].copy_from_slice(&0x0022u16.to_le_bytes()); // EXECUTABLE | LARGE_ADDRESS_AWARE
                                                                           // Optional header (PE32+).
        let opt = coff + 20;
        b[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes()); // PE32+
        b[opt + 16..opt + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // entry point
        b[opt + 24..opt + 32].copy_from_slice(&0x1_4000_0000u64.to_le_bytes()); // image base
        b[opt + 56..opt + 60].copy_from_slice(&0x4000u32.to_le_bytes()); // size of image
        b[opt + 68..opt + 70].copy_from_slice(&3u16.to_le_bytes()); // subsystem console
        b[opt + 70..opt + 72].copy_from_slice(&0x0160u16.to_le_bytes()); // dyn base | nx | force integ
        b[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes()); // NumberOfRvaAndSizes

        // Section table after optional header.
        let sec = opt + opt_size as usize;
        b[sec..sec + 5].copy_from_slice(b".text");
        b[sec + 8..sec + 12].copy_from_slice(&0x200u32.to_le_bytes()); // virtual size
        b[sec + 12..sec + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // virtual address
        b[sec + 16..sec + 20].copy_from_slice(&0x200u32.to_le_bytes()); // raw size
        b[sec + 20..sec + 24].copy_from_slice(&0x400u32.to_le_bytes()); // raw ptr
        b[sec + 36..sec + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes()); // code|exec|read

        // Build the import table inside the .text section's raw data (file
        // offset 0x400, RVA 0x1000). We lay out:
        //   IDT entry @ rva 0x1000
        //   ILT (thunks) @ rva 0x1040
        //   IMAGE_IMPORT_BY_NAME @ rva 0x1060
        //   dll name @ rva 0x1080
        let raw = 0x400usize;
        let rva_base = 0x1000u32;
        let off = |rva: u32| raw + (rva - rva_base) as usize;
        // IDT entry: OriginalFirstThunk=0x1040, Name=0x1080, FirstThunk=0x1040
        let idt = off(0x1000);
        b[idt..idt + 4].copy_from_slice(&0x1040u32.to_le_bytes()); // OriginalFirstThunk
        b[idt + 12..idt + 16].copy_from_slice(&0x1080u32.to_le_bytes()); // Name
        b[idt + 16..idt + 20].copy_from_slice(&0x1040u32.to_le_bytes()); // FirstThunk
                                                                         // second IDT entry zeroed (terminator) — already zero.
                                                                         // ILT thunk (PE32+ = 8 bytes): points to IMAGE_IMPORT_BY_NAME at 0x1060.
        let ilt = off(0x1040);
        b[ilt..ilt + 8].copy_from_slice(&0x1060u64.to_le_bytes());
        // terminator thunk at 0x1048 already zero.
        // IMAGE_IMPORT_BY_NAME: 2-byte hint + name.
        let ibn = off(0x1060);
        b[ibn..ibn + 2].copy_from_slice(&0u16.to_le_bytes());
        b[ibn + 2..ibn + 2 + 11].copy_from_slice(b"ExitProcess");
        // DLL name.
        let dll = off(0x1080);
        b[dll..dll + 12].copy_from_slice(b"KERNEL32.dll");

        // Import data directory (index 1): RVA + size.
        let import_dir = opt + 112 + 1 * 8;
        b[import_dir..import_dir + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        b[import_dir + 4..import_dir + 8].copy_from_slice(&40u32.to_le_bytes());

        b
    }

    #[test]
    fn parses_minimal_pe32plus_with_imports() {
        let bytes = build_pe();
        let pe = parse(&bytes).expect("should parse");
        assert_eq!(pe.format, "PE32+");
        assert_eq!(pe.machine, "x86-64");
        assert!(!pe.is_dll);
        assert_eq!(pe.kind, "Executable");
        assert_eq!(pe.subsystem, "Windows Console");
        assert_eq!(pe.section_count, 1);
        assert_eq!(pe.image_base, 0x1_4000_0000);
        assert_eq!(pe.entry_point, 0x1000);
        assert_eq!(pe.sections.len(), 1);
        assert_eq!(pe.sections[0].name, ".text");
        assert!(pe.sections[0].flags.contains(&"code"));
        assert!(pe.dll_characteristics.contains(&"dynamic base (ASLR)"));
        assert!(pe.dll_characteristics.contains(&"NX compatible (DEP)"));
        assert_eq!(pe.imports.len(), 1);
        assert_eq!(pe.imports[0].name, "KERNEL32.dll");
        assert_eq!(pe.imports[0].functions, vec!["ExitProcess".to_string()]);
        assert!(pe.exports.is_empty());
    }

    #[test]
    fn rejects_non_pe() {
        assert!(parse(b"not a pe at all, just text padding padding").is_err());
        assert!(parse(b"MZ").is_err()); // too small
        let mut almost = vec![0u8; 128];
        almost[0] = b'M';
        almost[1] = b'Z';
        // e_lfanew points somewhere with no PE signature.
        almost[0x3c] = 0x40;
        assert!(parse(&almost).is_err());
    }

    #[test]
    fn machine_and_subsystem_tables() {
        assert_eq!(machine_name(0x014c), "x86");
        assert_eq!(machine_name(0xaa64), "ARM64");
        assert_eq!(subsystem_name(2), "Windows GUI");
        assert_eq!(subsystem_name(1), "Native");
    }
}

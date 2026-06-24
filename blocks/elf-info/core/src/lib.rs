//! gizza-ai/elf-info core — pure, dependency-free parser for the ELF
//! (Executable and Linkable Format) container used by Linux/Unix executables,
//! shared objects (`.so`), relocatable objects (`.o`), and core dumps.
//!
//! Parses the ELF identification + file header (ELF32 / ELF64, little- or
//! big-endian), the section header table, the program header (segment) table,
//! and the symbol table(s) (`.symtab` / `.dynsym`). No external crates → it
//! instantiates cleanly in the wafer (wasm32-wasip1) runtime and runs on every
//! backend including the chat Service Worker.
//!
//! References: System V ABI / `elf.h` (Elf32_Ehdr, Elf64_Ehdr, Elf*_Shdr,
//! Elf*_Phdr, Elf*_Sym).

/// Parsed summary of an ELF binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfInfo {
    /// `ELF32` or `ELF64`, from `EI_CLASS`.
    pub class: &'static str,
    /// `little-endian` or `big-endian`, from `EI_DATA`.
    pub endianness: &'static str,
    /// ELF header version (`EI_VERSION`), normally 1.
    pub version: u8,
    /// Target OS/ABI, e.g. `System V`, `Linux`, `FreeBSD`.
    pub os_abi: &'static str,
    /// ABI version byte (`EI_ABIVERSION`).
    pub abi_version: u8,
    /// File type, e.g. `Executable`, `Shared object`, `Relocatable`, `Core`.
    pub file_type: &'static str,
    /// Target machine architecture, e.g. `x86-64`, `AArch64`, `RISC-V`.
    pub machine: String,
    /// Whether this is a position-independent executable / shared object
    /// (`ET_DYN`).
    pub is_pie_or_shared: bool,
    /// Entry-point virtual address.
    pub entry_point: u64,
    /// Number of program headers (segments).
    pub program_header_count: u16,
    /// Number of section headers.
    pub section_header_count: u16,
    /// Decoded ELF header flags (architecture-specific), if any are notable.
    pub flags: Vec<String>,
    /// The `PT_INTERP` requested program interpreter / dynamic linker, if any.
    pub interpreter: Option<String>,
    /// Whether a `PT_DYNAMIC` segment is present (dynamically linked).
    pub is_dynamic: bool,
    /// One entry per section header.
    pub sections: Vec<Section>,
    /// One entry per program header (segment).
    pub segments: Vec<Segment>,
    /// Parsed symbols from `.symtab` and/or `.dynsym`.
    pub symbols: Vec<Symbol>,
    /// True if the symbol list was truncated to the cap.
    pub symbols_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    /// Section type, e.g. `PROGBITS`, `SYMTAB`, `STRTAB`, `NOBITS`.
    pub sh_type: String,
    /// Decoded section flags, e.g. `write`, `alloc`, `execinstr`.
    pub flags: Vec<&'static str>,
    pub address: u64,
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Segment type, e.g. `LOAD`, `DYNAMIC`, `INTERP`, `GNU_STACK`.
    pub p_type: String,
    /// Decoded permission flags: `read`, `write`, `execute`.
    pub flags: Vec<&'static str>,
    pub offset: u64,
    pub virtual_address: u64,
    pub file_size: u64,
    pub memory_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    /// Symbol type, e.g. `FUNC`, `OBJECT`, `SECTION`, `FILE`, `NOTYPE`.
    pub kind: &'static str,
    /// Binding, e.g. `LOCAL`, `GLOBAL`, `WEAK`.
    pub binding: &'static str,
    pub value: u64,
    pub size: u64,
    /// Which table it came from: `.symtab` or `.dynsym`.
    pub table: &'static str,
}

// Caps so a huge binary cannot produce megabytes of JSON.
const MAX_SECTIONS: usize = 2048;
const MAX_SEGMENTS: usize = 1024;
const MAX_SYMBOLS: usize = 8192;

/// Endianness-aware integer readers over a byte slice.
#[derive(Clone, Copy)]
struct Reader {
    le: bool,
}

impl Reader {
    fn u16(&self, b: &[u8], off: usize) -> Option<u16> {
        let s = b.get(off..off + 2)?;
        Some(if self.le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        })
    }
    fn u32(&self, b: &[u8], off: usize) -> Option<u32> {
        let s = b.get(off..off + 4)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }
    fn u64(&self, b: &[u8], off: usize) -> Option<u64> {
        let s = b.get(off..off + 8)?;
        let a = [s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]];
        Some(if self.le {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        })
    }
}

fn os_abi_name(v: u8) -> &'static str {
    match v {
        0 => "System V",
        1 => "HP-UX",
        2 => "NetBSD",
        3 => "Linux (GNU)",
        4 => "GNU Hurd",
        6 => "Solaris",
        7 => "AIX",
        8 => "IRIX",
        9 => "FreeBSD",
        10 => "Tru64",
        11 => "Novell Modesto",
        12 => "OpenBSD",
        13 => "OpenVMS",
        14 => "NonStop Kernel",
        15 => "AROS",
        16 => "FenixOS",
        17 => "Nuxi CloudABI",
        18 => "Stratus OpenVOS",
        64 => "ARM EABI",
        97 => "ARM",
        255 => "Standalone (embedded)",
        _ => "unknown",
    }
}

fn file_type_name(t: u16) -> &'static str {
    match t {
        0 => "None",
        1 => "Relocatable",
        2 => "Executable",
        3 => "Shared object",
        4 => "Core",
        0xfe00..=0xfeff => "OS-specific",
        0xff00..=0xffff => "Processor-specific",
        _ => "unknown",
    }
}

fn machine_name(m: u16) -> String {
    let s = match m {
        0 => "None",
        2 => "SPARC",
        3 => "x86 (i386)",
        4 => "Motorola 68000",
        7 => "Intel 80860",
        8 => "MIPS",
        18 => "SPARC32+",
        20 => "PowerPC",
        21 => "PowerPC 64-bit",
        22 => "IBM S/390",
        40 => "ARM",
        41 => "DEC Alpha",
        42 => "SuperH",
        43 => "SPARC v9 (64-bit)",
        50 => "Itanium (IA-64)",
        62 => "x86-64",
        76 => "OpenRISC",
        83 => "AVR",
        93 => "AVR32",
        94 => "Renesas RX",
        140 => "Renesas SH",
        164 => "Renesas RH850",
        183 => "AArch64 (ARM64)",
        188 => "Tilera TILEPro",
        190 => "CUDA",
        191 => "Tilera TILE-Gx",
        220 => "Zilog Z80",
        243 => "RISC-V",
        247 => "Linux BPF",
        258 => "LoongArch",
        other => return format!("unknown (0x{other:x})"),
    };
    s.to_string()
}

fn sh_type_name(t: u32) -> String {
    let s = match t {
        0 => "NULL",
        1 => "PROGBITS",
        2 => "SYMTAB",
        3 => "STRTAB",
        4 => "RELA",
        5 => "HASH",
        6 => "DYNAMIC",
        7 => "NOTE",
        8 => "NOBITS",
        9 => "REL",
        10 => "SHLIB",
        11 => "DYNSYM",
        14 => "INIT_ARRAY",
        15 => "FINI_ARRAY",
        16 => "PREINIT_ARRAY",
        17 => "GROUP",
        18 => "SYMTAB_SHNDX",
        0x6fff_fff6 => "GNU_HASH",
        0x6fff_fffd => "GNU_verdef",
        0x6fff_fffe => "GNU_verneed",
        0x6fff_ffff => "GNU_versym",
        other => return format!("0x{other:x}"),
    };
    s.to_string()
}

fn sh_flags(f: u64) -> Vec<&'static str> {
    let mut v = Vec::new();
    if f & 0x1 != 0 {
        v.push("write");
    }
    if f & 0x2 != 0 {
        v.push("alloc");
    }
    if f & 0x4 != 0 {
        v.push("execinstr");
    }
    if f & 0x10 != 0 {
        v.push("merge");
    }
    if f & 0x20 != 0 {
        v.push("strings");
    }
    if f & 0x40 != 0 {
        v.push("info-link");
    }
    if f & 0x80 != 0 {
        v.push("link-order");
    }
    if f & 0x200 != 0 {
        v.push("group");
    }
    if f & 0x400 != 0 {
        v.push("tls");
    }
    if f & 0x800 != 0 {
        v.push("compressed");
    }
    v
}

fn p_type_name(t: u32) -> String {
    let s = match t {
        0 => "NULL",
        1 => "LOAD",
        2 => "DYNAMIC",
        3 => "INTERP",
        4 => "NOTE",
        5 => "SHLIB",
        6 => "PHDR",
        7 => "TLS",
        0x6474_e550 => "GNU_EH_FRAME",
        0x6474_e551 => "GNU_STACK",
        0x6474_e552 => "GNU_RELRO",
        0x6474_e553 => "GNU_PROPERTY",
        other => return format!("0x{other:x}"),
    };
    s.to_string()
}

fn p_flags(f: u32) -> Vec<&'static str> {
    let mut v = Vec::new();
    if f & 0x4 != 0 {
        v.push("read");
    }
    if f & 0x2 != 0 {
        v.push("write");
    }
    if f & 0x1 != 0 {
        v.push("execute");
    }
    v
}

fn sym_type_name(t: u8) -> &'static str {
    match t {
        0 => "NOTYPE",
        1 => "OBJECT",
        2 => "FUNC",
        3 => "SECTION",
        4 => "FILE",
        5 => "COMMON",
        6 => "TLS",
        10 => "GNU_IFUNC",
        _ => "unknown",
    }
}

fn sym_binding_name(b: u8) -> &'static str {
    match b {
        0 => "LOCAL",
        1 => "GLOBAL",
        2 => "WEAK",
        10 => "GNU_UNIQUE",
        _ => "unknown",
    }
}

/// Read a NUL-terminated string from a string-table region at `base + idx`.
fn str_at(b: &[u8], base: usize, idx: u32) -> String {
    let start = base.saturating_add(idx as usize);
    let slice = match b.get(start..) {
        Some(s) => s,
        None => return String::new(),
    };
    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

/// Parse an ELF binary from its raw bytes. Returns a descriptive error if the
/// bytes are not a valid ELF image.
pub fn parse(b: &[u8]) -> Result<ElfInfo, String> {
    if b.len() < 16 {
        return Err("file too small to be an ELF binary".into());
    }
    if &b[0..4] != b"\x7fELF" {
        return Err("not an ELF file: missing the 0x7f 'E' 'L' 'F' magic".into());
    }
    let class = match b[4] {
        1 => "ELF32",
        2 => "ELF64",
        other => {
            return Err(format!(
                "invalid EI_CLASS {other} (expected 1=ELF32 or 2=ELF64)"
            ))
        }
    };
    let is64 = b[4] == 2;
    let (endianness, le) = match b[5] {
        1 => ("little-endian", true),
        2 => ("big-endian", false),
        other => return Err(format!("invalid EI_DATA {other} (expected 1=LE or 2=BE)")),
    };
    let r = Reader { le };
    let version = b[6];
    let os_abi = os_abi_name(b[7]);
    let abi_version = b[8];

    // File header fields. ELF32 and ELF64 share the same field order but with
    // different widths for addr/offset fields.
    let e_type = r.u16(b, 16).ok_or("truncated ELF header (e_type)")?;
    let e_machine = r.u16(b, 18).ok_or("truncated ELF header (e_machine)")?;

    let (entry_point, e_phoff, e_shoff, e_flags_off) = if is64 {
        (
            r.u64(b, 24).ok_or("truncated ELF header (e_entry)")?,
            r.u64(b, 32).ok_or("truncated ELF header (e_phoff)")? as usize,
            r.u64(b, 40).ok_or("truncated ELF header (e_shoff)")? as usize,
            48usize,
        )
    } else {
        (
            r.u32(b, 24).ok_or("truncated ELF header (e_entry)")? as u64,
            r.u32(b, 28).ok_or("truncated ELF header (e_phoff)")? as usize,
            r.u32(b, 32).ok_or("truncated ELF header (e_shoff)")? as usize,
            36usize,
        )
    };
    let e_flags = r.u32(b, e_flags_off).unwrap_or(0);
    // e_ehsize, e_phentsize, e_phnum, e_shentsize, e_shnum, e_shstrndx follow.
    let phent_off = e_flags_off + 6;
    let e_phentsize = r.u16(b, phent_off).unwrap_or(0) as usize;
    let e_phnum = r.u16(b, phent_off + 2).unwrap_or(0);
    let e_shentsize = r.u16(b, phent_off + 4).unwrap_or(0) as usize;
    let e_shnum = r.u16(b, phent_off + 6).unwrap_or(0);
    let e_shstrndx = r.u16(b, phent_off + 8).unwrap_or(0);

    // --- Section headers ---
    // Locate the section-header string table (.shstrtab) for section names.
    let shdr = |i: usize| -> Option<usize> {
        if e_shoff == 0 || e_shentsize == 0 {
            return None;
        }
        Some(e_shoff + i * e_shentsize)
    };
    let shstr_base = shdr(e_shstrndx as usize)
        .and_then(|o| {
            // sh_offset is at +16 (ELF32) / +24 (ELF64).
            if is64 {
                r.u64(b, o + 24)
            } else {
                r.u32(b, o + 16).map(|v| v as u64)
            }
        })
        .map(|v| v as usize);

    let mut sections = Vec::new();
    // For symbol parsing: record (sh_type, sh_offset, sh_size, sh_link, sh_entsize).
    struct RawShdr {
        sh_type: u32,
        offset: u64,
        size: u64,
        link: u32,
        entsize: u64,
    }
    let mut raw_shdrs: Vec<RawShdr> = Vec::new();

    for i in 0..(e_shnum as usize).min(MAX_SECTIONS) {
        let o = match shdr(i) {
            Some(o) => o,
            None => break,
        };
        if b.get(o..o + e_shentsize.max(1)).is_none() {
            break;
        }
        let sh_name = r.u32(b, o).unwrap_or(0);
        let sh_type = r.u32(b, o + 4).unwrap_or(0);
        let (sh_flags_v, sh_addr, sh_off, sh_size, sh_link, sh_entsize) = if is64 {
            (
                r.u64(b, o + 8).unwrap_or(0),
                r.u64(b, o + 16).unwrap_or(0),
                r.u64(b, o + 24).unwrap_or(0),
                r.u64(b, o + 32).unwrap_or(0),
                r.u32(b, o + 40).unwrap_or(0),
                r.u64(b, o + 56).unwrap_or(0),
            )
        } else {
            (
                r.u32(b, o + 8).unwrap_or(0) as u64,
                r.u32(b, o + 12).unwrap_or(0) as u64,
                r.u32(b, o + 16).unwrap_or(0) as u64,
                r.u32(b, o + 20).unwrap_or(0) as u64,
                r.u32(b, o + 24).unwrap_or(0),
                r.u32(b, o + 36).unwrap_or(0) as u64,
            )
        };
        let name = match shstr_base {
            Some(base) => str_at(b, base, sh_name),
            None => String::new(),
        };
        sections.push(Section {
            name,
            sh_type: sh_type_name(sh_type),
            flags: sh_flags(sh_flags_v),
            address: sh_addr,
            offset: sh_off,
            size: sh_size,
        });
        raw_shdrs.push(RawShdr {
            sh_type,
            offset: sh_off,
            size: sh_size,
            link: sh_link,
            entsize: sh_entsize,
        });
    }

    // --- Program headers (segments) ---
    let mut segments = Vec::new();
    let mut interpreter = None;
    let mut is_dynamic = false;
    if e_phoff != 0 && e_phentsize != 0 {
        for i in 0..(e_phnum as usize).min(MAX_SEGMENTS) {
            let o = e_phoff + i * e_phentsize;
            if b.get(o..o + e_phentsize).is_none() {
                break;
            }
            // ELF32 and ELF64 program headers order fields differently.
            let (p_type, p_flags_v, p_offset, p_vaddr, p_filesz, p_memsz) = if is64 {
                (
                    r.u32(b, o).unwrap_or(0),
                    r.u32(b, o + 4).unwrap_or(0),
                    r.u64(b, o + 8).unwrap_or(0),
                    r.u64(b, o + 16).unwrap_or(0),
                    r.u64(b, o + 32).unwrap_or(0),
                    r.u64(b, o + 40).unwrap_or(0),
                )
            } else {
                (
                    r.u32(b, o).unwrap_or(0),
                    r.u32(b, o + 24).unwrap_or(0),
                    r.u32(b, o + 4).unwrap_or(0) as u64,
                    r.u32(b, o + 8).unwrap_or(0) as u64,
                    r.u32(b, o + 16).unwrap_or(0) as u64,
                    r.u32(b, o + 20).unwrap_or(0) as u64,
                )
            };
            if p_type == 2 {
                is_dynamic = true;
            }
            if p_type == 3 {
                // PT_INTERP: the segment data is a NUL-terminated path.
                interpreter = b
                    .get(p_offset as usize..(p_offset + p_filesz) as usize)
                    .map(|s| {
                        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                        String::from_utf8_lossy(&s[..end]).into_owned()
                    })
                    .filter(|s| !s.is_empty());
            }
            segments.push(Segment {
                p_type: p_type_name(p_type),
                flags: p_flags(p_flags_v),
                offset: p_offset,
                virtual_address: p_vaddr,
                file_size: p_filesz,
                memory_size: p_memsz,
            });
        }
    }

    // --- Symbol tables (.symtab / .dynsym) ---
    let mut symbols = Vec::new();
    let mut symbols_truncated = false;
    let sym_entsize = if is64 { 24usize } else { 16usize };
    for sh in &raw_shdrs {
        // 2 = SYMTAB, 11 = DYNSYM.
        let table = match sh.sh_type {
            2 => ".symtab",
            11 => ".dynsym",
            _ => continue,
        };
        let entsize = if sh.entsize != 0 {
            sh.entsize as usize
        } else {
            sym_entsize
        };
        if entsize == 0 {
            continue;
        }
        // The linked section is the associated string table.
        let strtab_base = raw_shdrs
            .get(sh.link as usize)
            .map(|s| s.offset as usize)
            .unwrap_or(0);
        let count = (sh.size as usize) / entsize;
        let base = sh.offset as usize;
        for i in 0..count {
            if symbols.len() >= MAX_SYMBOLS {
                symbols_truncated = true;
                break;
            }
            let o = base + i * entsize;
            if b.get(o..o + entsize).is_none() {
                break;
            }
            // Field layout differs between ELF32 and ELF64.
            let (st_name, st_info, st_value, st_size) = if is64 {
                (
                    r.u32(b, o).unwrap_or(0),
                    *b.get(o + 4).unwrap_or(&0),
                    r.u64(b, o + 8).unwrap_or(0),
                    r.u64(b, o + 16).unwrap_or(0),
                )
            } else {
                (
                    r.u32(b, o).unwrap_or(0),
                    *b.get(o + 12).unwrap_or(&0),
                    r.u32(b, o + 4).unwrap_or(0) as u64,
                    r.u32(b, o + 8).unwrap_or(0) as u64,
                )
            };
            // The first symbol (index 0) is the reserved null symbol.
            if i == 0 && st_name == 0 && st_value == 0 {
                continue;
            }
            let name = str_at(b, strtab_base, st_name);
            symbols.push(Symbol {
                name,
                kind: sym_type_name(st_info & 0xf),
                binding: sym_binding_name(st_info >> 4),
                value: st_value,
                size: st_size,
                table,
            });
        }
        if symbols.len() >= MAX_SYMBOLS {
            symbols_truncated = true;
        }
    }

    let flags = decode_machine_flags(e_machine, e_flags);

    Ok(ElfInfo {
        class,
        endianness,
        version,
        os_abi,
        abi_version,
        file_type: file_type_name(e_type),
        machine: machine_name(e_machine),
        is_pie_or_shared: e_type == 3,
        entry_point,
        program_header_count: e_phnum,
        section_header_count: e_shnum,
        flags,
        interpreter,
        is_dynamic,
        sections,
        segments,
        symbols,
        symbols_truncated,
    })
}

/// Decode a few of the better-known architecture-specific `e_flags`. For most
/// architectures e_flags is 0; we surface notable ARM/MIPS/RISC-V bits.
fn decode_machine_flags(machine: u16, f: u32) -> Vec<String> {
    let mut v = Vec::new();
    match machine {
        40 => {
            // ARM
            let abi = (f & 0xff00_0000) >> 24;
            if abi != 0 {
                v.push(format!("ARM EABI version {abi}"));
            }
            if f & 0x0040_0000 != 0 {
                v.push("hard-float ABI".to_string());
            }
            if f & 0x0020_0000 != 0 {
                v.push("soft-float ABI".to_string());
            }
        }
        243 => {
            // RISC-V
            if f & 0x1 != 0 {
                v.push("RVC (compressed)".to_string());
            }
            match (f & 0x6) >> 1 {
                1 => v.push("single-float ABI".to_string()),
                2 => v.push("double-float ABI".to_string()),
                3 => v.push("quad-float ABI".to_string()),
                _ => {}
            }
        }
        8 => {
            // MIPS
            if f & 0x2000_0000 != 0 {
                v.push("PIC".to_string());
            }
            if f & 0x4000_0000 != 0 {
                v.push("CPIC".to_string());
            }
        }
        _ => {
            if f != 0 {
                v.push(format!("0x{f:x}"));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal but structurally valid little-endian ELF64 executable:
    /// ELF header → one PT_LOAD program header → a `.text` PROGBITS + a
    /// `.shstrtab` STRTAB section → no symbols.
    fn build_elf64() -> Vec<u8> {
        let mut b = vec![0u8; 0x400];
        // e_ident.
        b[0..4].copy_from_slice(b"\x7fELF");
        b[4] = 2; // ELF64
        b[5] = 1; // little-endian
        b[6] = 1; // version
        b[7] = 0; // System V
        b[8] = 0; // ABI version
                  // e_type = ET_EXEC(2), e_machine = x86-64(62).
        b[16..18].copy_from_slice(&2u16.to_le_bytes());
        b[18..20].copy_from_slice(&62u16.to_le_bytes());
        b[20..24].copy_from_slice(&1u32.to_le_bytes()); // e_version
        b[24..32].copy_from_slice(&0x40_1000u64.to_le_bytes()); // e_entry
        let e_phoff: u64 = 0x40;
        b[32..40].copy_from_slice(&e_phoff.to_le_bytes());
        let e_shoff: u64 = 0x100;
        b[40..48].copy_from_slice(&e_shoff.to_le_bytes());
        b[48..52].copy_from_slice(&0u32.to_le_bytes()); // e_flags
                                                        // e_ehsize, e_phentsize=56, e_phnum=1, e_shentsize=64, e_shnum=2, e_shstrndx=1.
        b[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
        b[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
        b[56..58].copy_from_slice(&1u16.to_le_bytes()); // e_phnum
        b[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
        b[60..62].copy_from_slice(&2u16.to_le_bytes()); // e_shnum
        b[62..64].copy_from_slice(&1u16.to_le_bytes()); // e_shstrndx

        // Program header at 0x40 (PT_LOAD, R+X).
        let p = 0x40usize;
        b[p..p + 4].copy_from_slice(&1u32.to_le_bytes()); // p_type LOAD
        b[p + 4..p + 8].copy_from_slice(&5u32.to_le_bytes()); // p_flags R|X
        b[p + 8..p + 16].copy_from_slice(&0u64.to_le_bytes()); // p_offset
        b[p + 16..p + 24].copy_from_slice(&0x40_0000u64.to_le_bytes()); // p_vaddr
        b[p + 32..p + 40].copy_from_slice(&0x200u64.to_le_bytes()); // p_filesz
        b[p + 40..p + 48].copy_from_slice(&0x200u64.to_le_bytes()); // p_memsz

        // .shstrtab string table content at 0x200: "\0.text\0.shstrtab\0".
        let strtab = 0x200usize;
        // index 0 = "" ; ".text" at 1 ; ".shstrtab" at 7.
        let names = b"\0.text\0.shstrtab\0";
        b[strtab..strtab + names.len()].copy_from_slice(names);

        // Section header 0 (index 0): .text PROGBITS, alloc+exec.
        let sh0 = e_shoff as usize;
        b[sh0..sh0 + 4].copy_from_slice(&1u32.to_le_bytes()); // sh_name -> ".text"
        b[sh0 + 4..sh0 + 8].copy_from_slice(&1u32.to_le_bytes()); // sh_type PROGBITS
        b[sh0 + 8..sh0 + 16].copy_from_slice(&0x6u64.to_le_bytes()); // flags alloc|exec
        b[sh0 + 16..sh0 + 24].copy_from_slice(&0x40_1000u64.to_le_bytes()); // sh_addr
        b[sh0 + 24..sh0 + 32].copy_from_slice(&0x1000u64.to_le_bytes()); // sh_offset
        b[sh0 + 32..sh0 + 40].copy_from_slice(&0x200u64.to_le_bytes()); // sh_size

        // Section header 1 (index 1 = shstrtab): STRTAB.
        let sh1 = sh0 + 64;
        b[sh1..sh1 + 4].copy_from_slice(&7u32.to_le_bytes()); // sh_name -> ".shstrtab"
        b[sh1 + 4..sh1 + 8].copy_from_slice(&3u32.to_le_bytes()); // sh_type STRTAB
        b[sh1 + 16..sh1 + 24].copy_from_slice(&0u64.to_le_bytes()); // sh_addr
        b[sh1 + 24..sh1 + 32].copy_from_slice(&(strtab as u64).to_le_bytes()); // sh_offset
        b[sh1 + 32..sh1 + 40].copy_from_slice(&(names.len() as u64).to_le_bytes()); // sh_size

        b
    }

    #[test]
    fn parses_minimal_elf64() {
        let bytes = build_elf64();
        let elf = parse(&bytes).expect("should parse");
        assert_eq!(elf.class, "ELF64");
        assert_eq!(elf.endianness, "little-endian");
        assert_eq!(elf.os_abi, "System V");
        assert_eq!(elf.file_type, "Executable");
        assert_eq!(elf.machine, "x86-64");
        assert!(!elf.is_pie_or_shared);
        assert_eq!(elf.entry_point, 0x40_1000);
        assert_eq!(elf.program_header_count, 1);
        assert_eq!(elf.section_header_count, 2);
        // Sections.
        assert_eq!(elf.sections.len(), 2);
        assert_eq!(elf.sections[0].name, ".text");
        assert_eq!(elf.sections[0].sh_type, "PROGBITS");
        assert!(elf.sections[0].flags.contains(&"alloc"));
        assert!(elf.sections[0].flags.contains(&"execinstr"));
        assert_eq!(elf.sections[1].name, ".shstrtab");
        assert_eq!(elf.sections[1].sh_type, "STRTAB");
        // Segments.
        assert_eq!(elf.segments.len(), 1);
        assert_eq!(elf.segments[0].p_type, "LOAD");
        assert!(elf.segments[0].flags.contains(&"read"));
        assert!(elf.segments[0].flags.contains(&"execute"));
        assert!(!elf.segments[0].flags.contains(&"write"));
        assert_eq!(elf.segments[0].virtual_address, 0x40_0000);
        assert!(elf.symbols.is_empty());
        assert!(!elf.symbols_truncated);
    }

    #[test]
    fn parses_elf64_with_symbols() {
        // Extend the minimal ELF with a .symtab + .strtab and 4 section headers.
        let mut b = build_elf64();
        // Bump e_shnum to 4 and keep e_shstrndx = 1.
        b[60..62].copy_from_slice(&4u16.to_le_bytes());
        // .strtab content at 0x280: "\0main\0".
        let strtab = 0x280usize;
        let names = b"\0main\0";
        b[strtab..strtab + names.len()].copy_from_slice(names);
        // .symtab content at 0x2c0: 2 entries (null + "main" FUNC GLOBAL).
        let symtab = 0x2c0usize;
        // entry 0 = all zero (null symbol).
        // entry 1 at +24.
        let s1 = symtab + 24;
        b[s1..s1 + 4].copy_from_slice(&1u32.to_le_bytes()); // st_name -> "main"
        b[s1 + 4] = (1 << 4) | 2; // GLOBAL | FUNC
        b[s1 + 8..s1 + 16].copy_from_slice(&0x40_1000u64.to_le_bytes()); // st_value
        b[s1 + 16..s1 + 24].copy_from_slice(&0x20u64.to_le_bytes()); // st_size

        // Section header 2 = .symtab (SYMTAB, link -> 3 = .strtab).
        let e_shoff = 0x100usize;
        let sh2 = e_shoff + 2 * 64;
        b[sh2 + 4..sh2 + 8].copy_from_slice(&2u32.to_le_bytes()); // SYMTAB
        b[sh2 + 24..sh2 + 32].copy_from_slice(&(symtab as u64).to_le_bytes()); // offset
        b[sh2 + 32..sh2 + 40].copy_from_slice(&(48u64).to_le_bytes()); // size = 2*24
        b[sh2 + 40..sh2 + 44].copy_from_slice(&3u32.to_le_bytes()); // link -> 3
        b[sh2 + 56..sh2 + 64].copy_from_slice(&24u64.to_le_bytes()); // entsize

        // Section header 3 = .strtab (STRTAB).
        let sh3 = e_shoff + 3 * 64;
        b[sh3 + 4..sh3 + 8].copy_from_slice(&3u32.to_le_bytes()); // STRTAB
        b[sh3 + 24..sh3 + 32].copy_from_slice(&(strtab as u64).to_le_bytes()); // offset
        b[sh3 + 32..sh3 + 40].copy_from_slice(&(names.len() as u64).to_le_bytes()); // size

        let elf = parse(&b).expect("should parse");
        assert_eq!(elf.section_header_count, 4);
        assert_eq!(elf.symbols.len(), 1);
        assert_eq!(elf.symbols[0].name, "main");
        assert_eq!(elf.symbols[0].kind, "FUNC");
        assert_eq!(elf.symbols[0].binding, "GLOBAL");
        assert_eq!(elf.symbols[0].value, 0x40_1000);
        assert_eq!(elf.symbols[0].size, 0x20);
        assert_eq!(elf.symbols[0].table, ".symtab");
    }

    #[test]
    fn rejects_non_elf() {
        assert!(parse(b"not an elf at all, padding padding pad").is_err());
        assert!(parse(b"\x7fELF").is_err()); // too small
        let mut bad = vec![0u8; 64];
        bad[0..4].copy_from_slice(b"\x7fELF");
        bad[4] = 9; // invalid class
        assert!(parse(&bad).is_err());
    }

    #[test]
    fn tables_decode() {
        assert_eq!(machine_name(183), "AArch64 (ARM64)");
        assert_eq!(machine_name(62), "x86-64");
        assert_eq!(file_type_name(3), "Shared object");
        assert_eq!(os_abi_name(9), "FreeBSD");
        assert_eq!(sh_type_name(2), "SYMTAB");
        assert_eq!(p_type_name(1), "LOAD");
        assert_eq!(sym_type_name(2), "FUNC");
        assert_eq!(sym_binding_name(2), "WEAK");
    }

    #[test]
    fn big_endian_header() {
        // Build a tiny big-endian ELF32 header and confirm endianness handling.
        let mut b = vec![0u8; 64];
        b[0..4].copy_from_slice(b"\x7fELF");
        b[4] = 1; // ELF32
        b[5] = 2; // big-endian
        b[6] = 1;
        b[16..18].copy_from_slice(&2u16.to_be_bytes()); // e_type EXEC
        b[18..20].copy_from_slice(&20u16.to_be_bytes()); // PowerPC
        b[24..28].copy_from_slice(&0x10_000u32.to_be_bytes()); // e_entry
        let elf = parse(&b).expect("should parse");
        assert_eq!(elf.class, "ELF32");
        assert_eq!(elf.endianness, "big-endian");
        assert_eq!(elf.machine, "PowerPC");
        assert_eq!(elf.entry_point, 0x10_000);
    }
}

//! gizza-ai/elf-macho-info core — pure, dependency-free parser for the two Unix
//! executable containers: **ELF** (Linux/BSD/Solaris executables, `.so` shared
//! objects, `.o` relocatables, core dumps) and **Mach-O** (macOS/iOS
//! executables, `.dylib` libraries, bundles, kexts), including fat/universal
//! Mach-O archives that bundle several architecture slices in one file.
//!
//! Both formats are reduced to ONE [`BinaryInfo`] shape so a caller gets the
//! same fields — architecture, file type, sections, symbols, linked libraries,
//! imports/exports — whichever container it fed in.
//!
//! No external crates: every integer is read by hand through bounds-checked
//! slices, so the crate instantiates cleanly in the wafer (wasm32-wasip1)
//! runtime and runs on every backend including the chat Service Worker. Nothing
//! here can panic on hostile input — a truncated or lying header becomes an
//! `Err(String)` or a skipped entry, never an index-out-of-bounds.
//!
//! References: System V gABI / `elf.h` (`Elf*_Ehdr`, `Elf*_Shdr`, `Elf*_Phdr`,
//! `Elf*_Sym`, `Elf*_Dyn`) and Apple's `<mach-o/loader.h>` + `<mach-o/nlist.h>`
//! (`mach_header`, `mach_header_64`, `load_command`, `segment_command*`,
//! `section*`, `dylib_command`, `symtab_command`, `nlist*`, `fat_header`).

// ---------------------------------------------------------------------------
// Public model
// ---------------------------------------------------------------------------

/// What the caller wants back. Every list is independently switchable because
/// a stripped-down report (architecture + libraries only) is a common triage
/// need, and a full symbol table on a real binary is tens of thousands of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Include the section table.
    pub sections: bool,
    /// Include the symbol table(s).
    pub symbols: bool,
    /// Include linked libraries, imported symbols, and exported symbols.
    pub imports: bool,
    /// Max entries per list (sections, symbols, imports, exports). Clamped to
    /// [`MAX_LIMIT`].
    pub limit: usize,
    /// For a fat/universal Mach-O: which slice to report, by architecture name
    /// (`x86_64`, `arm64`, …). `None` reports the first slice.
    pub arch: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            sections: true,
            symbols: true,
            imports: true,
            limit: 100,
            arch: None,
        }
    }
}

/// Hard ceiling on `Options::limit` — a report is for humans and LLMs, not a
/// database dump.
pub const MAX_LIMIT: usize = 5000;

/// One section (ELF section header / Mach-O `section` inside a segment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// ELF: `.text`. Mach-O: `__TEXT,__text` (segment,section).
    pub name: String,
    /// ELF `sh_type` (`PROGBITS`, `SYMTAB`, `NOBITS`, …) or the Mach-O section
    /// type from the low byte of `flags` (`REGULAR`, `ZEROFILL`, …).
    pub kind: String,
    /// Decoded flags: ELF `write`/`alloc`/`execinstr`; Mach-O
    /// `pure-instructions`/`no-dead-strip`/….
    pub flags: Vec<String>,
    pub address: u64,
    /// File offset of the section's bytes (0 for ELF `NOBITS` / Mach-O
    /// zerofill).
    pub offset: u64,
    pub size: u64,
}

/// One symbol-table entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    /// ELF: `FUNC`/`OBJECT`/`SECTION`/`FILE`/`NOTYPE`/`TLS`. Mach-O:
    /// `SECT`/`UNDF`/`ABS`/`INDR`/`PBUD`/`STAB`.
    pub kind: String,
    /// ELF: `LOCAL`/`GLOBAL`/`WEAK`. Mach-O: `external`/`private external`/
    /// `local` (+ ` (weak)` when the descriptor says so).
    pub binding: String,
    pub value: u64,
    /// ELF `st_size`. Mach-O `nlist` carries no size, so always 0 there.
    pub size: u64,
    /// Which table it came from: `.symtab` / `.dynsym` / `LC_SYMTAB`.
    pub table: String,
}

/// One dynamically linked library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    /// ELF: the `DT_NEEDED` soname (`libc.so.6`). Mach-O: the install name
    /// (`/usr/lib/libSystem.B.dylib`).
    pub name: String,
    /// Where it came from: `NEEDED`, `LOAD_DYLIB`, `LOAD_WEAK_DYLIB`,
    /// `REEXPORT_DYLIB`, `LAZY_LOAD_DYLIB`, `LOAD_UPWARD_DYLIB`.
    pub kind: &'static str,
    /// Mach-O `current_version`, as `X.Y.Z`.
    pub current_version: Option<String>,
    /// Mach-O `compatibility_version`, as `X.Y.Z`.
    pub compatibility_version: Option<String>,
}

/// One architecture slice of a fat/universal Mach-O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    pub architecture: String,
    pub offset: u64,
    pub size: u64,
}

/// A capped list plus the pre-cap total, so a caller can say "showing 100 of
/// 41 233" instead of silently truncating.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Capped<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub truncated: bool,
}

impl<T> Capped<T> {
    fn build(mut items: Vec<T>, limit: usize) -> Self {
        let total = items.len();
        let truncated = total > limit;
        if truncated {
            items.truncate(limit);
        }
        Capped {
            items,
            total,
            truncated,
        }
    }
    fn empty() -> Self {
        Capped {
            items: Vec::new(),
            total: 0,
            truncated: false,
        }
    }
}

/// Unified summary of an ELF or Mach-O binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryInfo {
    /// `ELF` or `Mach-O`.
    pub format: &'static str,
    /// 32 or 64.
    pub bits: u8,
    /// `little-endian` or `big-endian`.
    pub endianness: &'static str,
    /// Target architecture, e.g. `x86-64`, `AArch64 (ARM64)`, `arm64e`.
    pub architecture: String,
    /// e.g. `Executable`, `Shared object`, `Dynamic library (dylib)`, `Bundle`.
    pub file_type: String,
    /// ELF `EI_OSABI`, e.g. `System V`, `Linux (GNU)`. `None` for Mach-O.
    pub os_abi: Option<String>,
    /// Mach-O deployment target, e.g. `macOS 13.0 (SDK 14.0)`. `None` for ELF.
    pub platform: Option<String>,
    /// Entry-point virtual address, when the container states one.
    pub entry_point: Option<u64>,
    /// Position-independent: ELF `ET_DYN`, Mach-O `MH_PIE`.
    pub is_pie: bool,
    /// Dynamically linked: ELF `PT_DYNAMIC`, Mach-O `MH_DYLDLINK`.
    pub is_dynamic: bool,
    /// No full symbol table (ELF `.symtab` absent, Mach-O `LC_SYMTAB` absent
    /// or empty).
    pub is_stripped: bool,
    /// ELF `PT_INTERP` program interpreter, or Mach-O `LC_LOAD_DYLINKER`.
    pub linker: Option<String>,
    /// ELF `DT_SONAME`, or Mach-O `LC_ID_DYLIB` install name.
    pub install_name: Option<String>,
    /// Mach-O `LC_UUID`, canonical 8-4-4-4-12 form.
    pub uuid: Option<String>,
    /// Decoded header flags (ELF machine-specific `e_flags`, Mach-O `MH_*`).
    pub flags: Vec<String>,
    /// ELF `DT_RPATH`/`DT_RUNPATH` entries, or Mach-O `LC_RPATH` paths.
    pub rpaths: Vec<String>,
    pub libraries: Capped<Library>,
    pub sections: Capped<Section>,
    pub symbols: Capped<Symbol>,
    /// Names of undefined (imported) symbols the dynamic linker must resolve.
    pub imports: Capped<String>,
    /// Names of defined external (exported) symbols.
    pub exports: Capped<String>,
    /// Slices of a fat/universal Mach-O; empty for a thin binary or an ELF.
    pub architectures: Vec<Slice>,
    /// Which fat slice this report describes.
    pub selected_architecture: Option<String>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Sniff the container from its magic and parse it.
///
/// Returns `Err` with a human-readable reason for empty, truncated, or
/// unrecognized input.
pub fn parse(bytes: &[u8], opts: &Options) -> Result<BinaryInfo, String> {
    let limit = opts.limit.clamp(1, MAX_LIMIT);
    let opts = Options {
        limit,
        arch: opts.arch.clone(),
        ..*opts
    };

    if bytes.is_empty() {
        return Err("input is empty — provide an ELF or Mach-O binary".to_string());
    }
    if bytes.len() < 4 {
        return Err(format!(
            "input is only {} byte(s) — too short to hold a 4-byte magic number",
            bytes.len()
        ));
    }

    if bytes.starts_with(b"\x7fELF") {
        return parse_elf(bytes, &opts);
    }

    let le = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let be = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    match le {
        MH_MAGIC | MH_MAGIC_64 | MH_CIGAM | MH_CIGAM_64 => return parse_macho(bytes, 0, &opts, &[]),
        _ => {}
    }
    if be == FAT_MAGIC || be == FAT_MAGIC_64 {
        return parse_fat(bytes, be == FAT_MAGIC_64, &opts);
    }

    Err(unrecognized(bytes))
}

/// Explain an unknown magic, and point at the right gizza tool when the bytes
/// are a container we deliberately do not handle here.
fn unrecognized(bytes: &[u8]) -> String {
    let hint = if bytes.starts_with(b"MZ") {
        " — this looks like a Windows PE/DOS executable; use the pe-info tool"
    } else if bytes.starts_with(b"PK\x03\x04") {
        " — this looks like a ZIP archive"
    } else if bytes.starts_with(b"!<arch>") {
        " — this looks like a static library / ar archive, not a single binary"
    } else if bytes.starts_with(b"\0asm") {
        " — this looks like a WebAssembly module"
    } else {
        ""
    };
    let head: Vec<String> = bytes.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!(
        "unrecognized binary: expected ELF (7f 45 4c 46) or Mach-O \
         (feedface / feedfacf / cafebabe universal) magic, found {}{hint}",
        head.join(" ")
    )
}

// ---------------------------------------------------------------------------
// Byte readers
// ---------------------------------------------------------------------------

/// Endianness-aware, bounds-checked integer reads over a byte slice.
#[derive(Clone, Copy)]
struct Reader {
    le: bool,
}

impl Reader {
    fn u16(&self, b: &[u8], off: usize) -> Option<u16> {
        let s = b.get(off..off.checked_add(2)?)?;
        let a = [s[0], s[1]];
        Some(if self.le {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        })
    }
    fn u32(&self, b: &[u8], off: usize) -> Option<u32> {
        let s = b.get(off..off.checked_add(4)?)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }
    fn u64(&self, b: &[u8], off: usize) -> Option<u64> {
        let s = b.get(off..off.checked_add(8)?)?;
        let a = [s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]];
        Some(if self.le {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        })
    }
    /// A 4- or 8-byte address/size depending on the container's word width.
    fn word(&self, b: &[u8], off: usize, wide: bool) -> Option<u64> {
        if wide {
            self.u64(b, off)
        } else {
            self.u32(b, off).map(u64::from)
        }
    }
}

/// Longest string we will pull out of a string table — a corrupt offset must
/// not turn into a megabyte of garbage.
const MAX_STR: usize = 4096;

/// NUL-terminated string at `off`, lossily decoded. Empty when out of range.
fn cstr(b: &[u8], off: usize) -> String {
    let Some(rest) = b.get(off..) else {
        return String::new();
    };
    let end = rest
        .iter()
        .take(MAX_STR)
        .position(|&c| c == 0)
        .unwrap_or_else(|| rest.len().min(MAX_STR));
    String::from_utf8_lossy(&rest[..end]).into_owned()
}

/// A fixed-width, optionally NUL-padded name field (Mach-O `segname` /
/// `sectname`, 16 bytes).
fn fixed_str(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

// ---------------------------------------------------------------------------
// ELF
// ---------------------------------------------------------------------------

const SHN_UNDEF: u16 = 0;
const SHN_XINDEX: u16 = 0xffff;
/// Refuse to walk absurd table lengths that a corrupt header claims.
const MAX_TABLE_ENTRIES: usize = 200_000;

fn parse_elf(b: &[u8], opts: &Options) -> Result<BinaryInfo, String> {
    if b.len() < 16 {
        return Err("truncated ELF: file is shorter than the 16-byte e_ident".to_string());
    }
    let (bits, wide) = match b[4] {
        1 => (32u8, false),
        2 => (64u8, true),
        other => return Err(format!("invalid ELF class byte {other} (expected 1=32-bit, 2=64-bit)")),
    };
    let (endianness, le) = match b[5] {
        1 => ("little-endian", true),
        2 => ("big-endian", false),
        other => {
            return Err(format!(
                "invalid ELF data-encoding byte {other} (expected 1=little-endian, 2=big-endian)"
            ))
        }
    };
    let r = Reader { le };
    let min_hdr = if wide { 64 } else { 52 };
    if b.len() < min_hdr {
        return Err(format!(
            "truncated ELF: file is {} bytes, shorter than the {min_hdr}-byte ELF{bits} header",
            b.len()
        ));
    }

    let e_type = r.u16(b, 16).unwrap_or(0);
    let e_machine = r.u16(b, 18).unwrap_or(0);
    let (entry_off, phoff_off, shoff_off, flags_off, ph_cnt_off, sh_cnt_off, shstrndx_off) = if wide
    {
        (24usize, 32usize, 40usize, 48usize, 56usize, 60usize, 62usize)
    } else {
        (24, 28, 32, 36, 44, 48, 50)
    };
    let entry = r.word(b, entry_off, wide).unwrap_or(0);
    let phoff = r.word(b, phoff_off, wide).unwrap_or(0);
    let shoff = r.word(b, shoff_off, wide).unwrap_or(0);
    let e_flags = r.u32(b, flags_off).unwrap_or(0);
    let phentsize = if wide { 56usize } else { 32 };
    let shentsize = if wide { 64usize } else { 40 };
    let phnum = r.u16(b, ph_cnt_off).unwrap_or(0) as usize;
    let mut shnum = r.u16(b, sh_cnt_off).unwrap_or(0) as usize;
    let mut shstrndx = r.u16(b, shstrndx_off).unwrap_or(0) as usize;

    // Extended section counts: when a file has ≥ 0xff00 sections, e_shnum is 0
    // and the real count lives in section 0's sh_size; likewise e_shstrndx
    // escapes to SHN_XINDEX with the real index in section 0's sh_link.
    let sh0 = shoff as usize;
    if shnum == 0 && shoff != 0 {
        shnum = r.word(b, sh0 + if wide { 32 } else { 20 }, wide).unwrap_or(0) as usize;
    }
    if shstrndx == SHN_XINDEX as usize && shoff != 0 {
        shstrndx = r.u32(b, sh0 + if wide { 40 } else { 24 }).unwrap_or(0) as usize;
    }
    let shnum = shnum.min(MAX_TABLE_ENTRIES);

    // --- program headers (segments) ---------------------------------------
    let mut interp: Option<String> = None;
    let mut is_dynamic = false;
    let mut dyn_span: Option<(u64, u64)> = None; // (file offset, size)
    let mut loads: Vec<(u64, u64, u64)> = Vec::new(); // (vaddr, offset, filesz)
    let phnum = phnum.min(MAX_TABLE_ENTRIES);
    for i in 0..phnum {
        let Some(base) = (phoff as usize).checked_add(i.checked_mul(phentsize).unwrap_or(usize::MAX))
        else {
            break;
        };
        let Some(p_type) = r.u32(b, base) else { break };
        let (off_o, vaddr_o, filesz_o) = if wide { (8, 16, 32) } else { (4, 8, 16) };
        let p_offset = r.word(b, base + off_o, wide).unwrap_or(0);
        let p_vaddr = r.word(b, base + vaddr_o, wide).unwrap_or(0);
        let p_filesz = r.word(b, base + filesz_o, wide).unwrap_or(0);
        match p_type {
            1 => loads.push((p_vaddr, p_offset, p_filesz)), // PT_LOAD
            2 => {
                // PT_DYNAMIC
                is_dynamic = true;
                dyn_span = Some((p_offset, p_filesz));
            }
            3 => interp = Some(cstr(b, p_offset as usize)), // PT_INTERP
            _ => {}
        }
    }

    // --- section headers ---------------------------------------------------
    struct Shdr {
        name_off: u32,
        sh_type: u32,
        flags: u64,
        addr: u64,
        offset: u64,
        size: u64,
        link: u32,
        entsize: u64,
    }
    let mut shdrs: Vec<Shdr> = Vec::new();
    for i in 0..shnum {
        let Some(base) = (shoff as usize).checked_add(i.checked_mul(shentsize).unwrap_or(usize::MAX))
        else {
            break;
        };
        let Some(name_off) = r.u32(b, base) else { break };
        let Some(sh_type) = r.u32(b, base + 4) else { break };
        let (flags_o, addr_o, off_o, size_o, link_o, ent_o) = if wide {
            (8usize, 16usize, 24usize, 32usize, 40usize, 56usize)
        } else {
            (8, 12, 16, 20, 24, 36)
        };
        shdrs.push(Shdr {
            name_off,
            sh_type,
            flags: r.word(b, base + flags_o, wide).unwrap_or(0),
            addr: r.word(b, base + addr_o, wide).unwrap_or(0),
            offset: r.word(b, base + off_o, wide).unwrap_or(0),
            size: r.word(b, base + size_o, wide).unwrap_or(0),
            link: r.u32(b, base + link_o).unwrap_or(0),
            entsize: r.word(b, base + ent_o, wide).unwrap_or(0),
        });
    }
    let shstr_base = shdrs.get(shstrndx).map(|s| s.offset as usize).unwrap_or(0);
    let sh_name = |s: &Shdr| -> String {
        if shstr_base == 0 && shstrndx == 0 {
            String::new()
        } else {
            cstr(b, shstr_base.saturating_add(s.name_off as usize))
        }
    };

    let sections: Vec<Section> = shdrs
        .iter()
        .map(|s| Section {
            name: sh_name(s),
            kind: elf_sh_type(s.sh_type),
            flags: elf_sh_flags(s.flags),
            address: s.addr,
            offset: if s.sh_type == 8 { 0 } else { s.offset }, // SHT_NOBITS
            size: s.size,
        })
        .collect();

    let is_stripped = !shdrs.iter().any(|s| s.sh_type == 2); // SHT_SYMTAB

    // --- symbols -----------------------------------------------------------
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut imports: Vec<String> = Vec::new();
    let mut exports: Vec<String> = Vec::new();
    let sym_entsize = if wide { 24usize } else { 16 };
    for (idx, s) in shdrs.iter().enumerate() {
        // SHT_SYMTAB = 2, SHT_DYNSYM = 11.
        let table: &'static str = match s.sh_type {
            2 => ".symtab",
            11 => ".dynsym",
            _ => continue,
        };
        let _ = idx;
        let ent = if s.entsize == 0 {
            sym_entsize
        } else {
            s.entsize as usize
        };
        if ent == 0 {
            continue;
        }
        let strtab_off = shdrs.get(s.link as usize).map(|t| t.offset as usize).unwrap_or(0);
        let count = ((s.size as usize) / ent).min(MAX_TABLE_ENTRIES);
        for i in 0..count {
            let Some(base) = (s.offset as usize).checked_add(i.checked_mul(ent).unwrap_or(usize::MAX))
            else {
                break;
            };
            let Some(name_off) = r.u32(b, base) else { break };
            let (info, shndx, value, size) = if wide {
                (
                    *b.get(base + 4).unwrap_or(&0),
                    r.u16(b, base + 6).unwrap_or(0),
                    r.u64(b, base + 8).unwrap_or(0),
                    r.u64(b, base + 16).unwrap_or(0),
                )
            } else {
                (
                    *b.get(base + 12).unwrap_or(&0),
                    r.u16(b, base + 14).unwrap_or(0),
                    r.u32(b, base + 4).unwrap_or(0) as u64,
                    r.u32(b, base + 8).unwrap_or(0) as u64,
                )
            };
            let name = cstr(b, strtab_off.saturating_add(name_off as usize));
            let binding = elf_binding(info >> 4);
            let kind = elf_sym_type(info & 0xf);
            if table == ".dynsym" && !name.is_empty() && binding != "LOCAL" {
                if shndx == SHN_UNDEF {
                    imports.push(name.clone());
                } else {
                    exports.push(name.clone());
                }
            }
            symbols.push(Symbol {
                name,
                kind: kind.to_string(),
                binding: binding.to_string(),
                value,
                size,
                table: table.to_string(),
            });
        }
    }

    // --- dynamic section: DT_NEEDED / DT_SONAME / DT_RPATH -----------------
    let mut libraries: Vec<Library> = Vec::new();
    let mut rpaths: Vec<String> = Vec::new();
    let mut install_name: Option<String> = None;
    // Prefer the .dynamic SECTION (present in every non-hostile file); fall
    // back to the PT_DYNAMIC segment when section headers were stripped.
    let dyn_region = shdrs
        .iter()
        .find(|s| s.sh_type == 6) // SHT_DYNAMIC
        .map(|s| (s.offset, s.size))
        .or(dyn_span);
    if let Some((doff, dsize)) = dyn_region {
        is_dynamic = true;
        // The string table the DT_* name offsets index into.
        let dynstr = shdrs
            .iter()
            .find(|s| s.sh_type == 3 && sh_name(s) == ".dynstr") // SHT_STRTAB
            .map(|s| s.offset as usize)
            .or_else(|| {
                // No sections: resolve DT_STRTAB's virtual address through the
                // PT_LOAD map.
                let strtab_va = dyn_entries(b, r, wide, doff, dsize).into_iter().find_map(
                    |(tag, val)| (tag == 5).then_some(val), // DT_STRTAB
                )?;
                vaddr_to_offset(&loads, strtab_va)
            })
            .unwrap_or(0);
        for (tag, val) in dyn_entries(b, r, wide, doff, dsize) {
            let name = || cstr(b, dynstr.saturating_add(val as usize));
            match tag {
                1 => libraries.push(Library {
                    // DT_NEEDED
                    name: name(),
                    kind: "NEEDED",
                    current_version: None,
                    compatibility_version: None,
                }),
                14 => install_name = Some(name()), // DT_SONAME
                15 | 29 => {
                    // DT_RPATH / DT_RUNPATH — colon-separated search paths.
                    rpaths.extend(name().split(':').filter(|p| !p.is_empty()).map(str::to_string));
                }
                _ => {}
            }
        }
    }

    let want_syms = opts.symbols;
    let want_imports = opts.imports;
    Ok(BinaryInfo {
        format: "ELF",
        bits,
        endianness,
        architecture: elf_machine(e_machine),
        file_type: elf_file_type(e_type).to_string(),
        os_abi: Some(elf_os_abi(b[7]).to_string()),
        platform: None,
        entry_point: Some(entry),
        is_pie: e_type == 3, // ET_DYN
        is_dynamic,
        is_stripped,
        linker: interp,
        install_name,
        uuid: None,
        flags: elf_header_flags(e_machine, e_flags),
        rpaths,
        libraries: if want_imports {
            Capped::build(libraries, opts.limit)
        } else {
            Capped::empty()
        },
        sections: if opts.sections {
            Capped::build(sections, opts.limit)
        } else {
            Capped::empty()
        },
        symbols: if want_syms {
            Capped::build(symbols, opts.limit)
        } else {
            Capped::empty()
        },
        imports: if want_imports {
            Capped::build(imports, opts.limit)
        } else {
            Capped::empty()
        },
        exports: if want_imports {
            Capped::build(exports, opts.limit)
        } else {
            Capped::empty()
        },
        architectures: Vec::new(),
        selected_architecture: None,
    })
}

/// Walk `.dynamic` / `PT_DYNAMIC` as `(d_tag, d_val)` pairs, stopping at
/// `DT_NULL`.
fn dyn_entries(b: &[u8], r: Reader, wide: bool, off: u64, size: u64) -> Vec<(u64, u64)> {
    let ent = if wide { 16usize } else { 8 };
    let count = ((size as usize) / ent).min(MAX_TABLE_ENTRIES);
    let mut out = Vec::new();
    for i in 0..count {
        let Some(base) = (off as usize).checked_add(i.saturating_mul(ent)) else {
            break;
        };
        let Some(tag) = r.word(b, base, wide) else { break };
        let Some(val) = r.word(b, base + ent / 2, wide) else {
            break;
        };
        if tag == 0 {
            break; // DT_NULL
        }
        out.push((tag, val));
    }
    out
}

/// Map a virtual address to a file offset using the `PT_LOAD` table.
fn vaddr_to_offset(loads: &[(u64, u64, u64)], vaddr: u64) -> Option<usize> {
    loads.iter().find_map(|&(va, off, filesz)| {
        (vaddr >= va && vaddr < va.saturating_add(filesz))
            .then(|| (off + (vaddr - va)) as usize)
    })
}

fn elf_file_type(t: u16) -> &'static str {
    match t {
        0 => "None",
        1 => "Relocatable object",
        2 => "Executable",
        3 => "Shared object / PIE",
        4 => "Core dump",
        0xfe00..=0xfeff => "OS-specific",
        0xff00..=0xffff => "Processor-specific",
        _ => "unknown",
    }
}

fn elf_os_abi(v: u8) -> &'static str {
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

fn elf_machine(m: u16) -> String {
    let s = match m {
        0 => "None",
        2 => "SPARC",
        3 => "x86 (i386)",
        4 => "Motorola 68000",
        8 => "MIPS",
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
        140 => "Renesas SH",
        183 => "AArch64 (ARM64)",
        190 => "CUDA",
        224 => "AMD GPU",
        243 => "RISC-V",
        247 => "BPF",
        252 => "C-SKY",
        258 => "LoongArch",
        _ => "",
    };
    if s.is_empty() {
        format!("unknown (e_machine {m})")
    } else {
        s.to_string()
    }
}

fn elf_sh_type(t: u32) -> String {
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
        0x6ffffff6 => "GNU_HASH",
        0x6ffffffd => "GNU_verdef",
        0x6ffffffe => "GNU_verneed",
        0x6fffffff => "GNU_versym",
        _ => "",
    };
    if s.is_empty() {
        format!("0x{t:x}")
    } else {
        s.to_string()
    }
}

fn elf_sh_flags(f: u64) -> Vec<String> {
    const NAMES: [(u64, &str); 11] = [
        (0x1, "write"),
        (0x2, "alloc"),
        (0x4, "execinstr"),
        (0x10, "merge"),
        (0x20, "strings"),
        (0x40, "info-link"),
        (0x80, "link-order"),
        (0x200, "group"),
        (0x400, "tls"),
        (0x800, "compressed"),
        (0x40000000, "exclude"),
    ];
    NAMES
        .iter()
        .filter(|(bit, _)| f & bit != 0)
        .map(|(_, n)| n.to_string())
        .collect()
}

fn elf_binding(b: u8) -> &'static str {
    match b {
        0 => "LOCAL",
        1 => "GLOBAL",
        2 => "WEAK",
        10..=12 => "OS-specific",
        13..=15 => "processor-specific",
        _ => "unknown",
    }
}

fn elf_sym_type(t: u8) -> &'static str {
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

/// The few `e_flags` bits worth naming. Most architectures leave `e_flags` 0;
/// ARM and RISC-V are the ones people actually read.
fn elf_header_flags(machine: u16, f: u32) -> Vec<String> {
    let mut out = Vec::new();
    match machine {
        40 => {
            // EM_ARM
            if f & 0x200 != 0 {
                out.push("soft-float ABI".to_string());
            }
            if f & 0x400 != 0 {
                out.push("hard-float ABI".to_string());
            }
            let ver = (f >> 24) & 0xff;
            if ver != 0 {
                out.push(format!("EABI v{ver}"));
            }
        }
        243 => {
            // EM_RISCV
            if f & 0x1 != 0 {
                out.push("RVC (compressed)".to_string());
            }
            match (f >> 1) & 0x3 {
                1 => out.push("float ABI: single".to_string()),
                2 => out.push("float ABI: double".to_string()),
                3 => out.push("float ABI: quad".to_string()),
                _ => {}
            }
            if f & 0x8 != 0 {
                out.push("RVE (embedded)".to_string());
            }
        }
        _ => {}
    }
    if out.is_empty() && f != 0 {
        out.push(format!("0x{f:x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Mach-O
// ---------------------------------------------------------------------------

const MH_MAGIC: u32 = 0xfeed_face;
const MH_CIGAM: u32 = 0xcefa_edfe;
const MH_MAGIC_64: u32 = 0xfeed_facf;
const MH_CIGAM_64: u32 = 0xcffa_edfe;
const FAT_MAGIC: u32 = 0xcafe_babe;
const FAT_MAGIC_64: u32 = 0xcafe_babf;

/// A universal ("fat") archive: a big-endian index of thin Mach-O slices.
fn parse_fat(b: &[u8], wide: bool, opts: &Options) -> Result<BinaryInfo, String> {
    let r = Reader { le: false };
    let nfat = r.u32(b, 4).unwrap_or(0) as usize;
    // `cafebabe` is also the Java class-file magic; a Java class stores its
    // minor/major version where nfat_arch lives, which is never a plausible
    // slice count. Reject rather than emit nonsense.
    if nfat == 0 || nfat > 64 {
        return Err(format!(
            "not a Mach-O universal binary: magic cafebabe but nfat_arch = {nfat} \
             (a Java .class file shares this magic)"
        ));
    }
    let ent = if wide { 32usize } else { 20 };
    let mut slices = Vec::new();
    for i in 0..nfat {
        let base = 8 + i * ent;
        let Some(cputype) = r.u32(b, base) else { break };
        let cpusubtype = r.u32(b, base + 4).unwrap_or(0);
        let (offset, size) = if wide {
            (
                r.u64(b, base + 8).unwrap_or(0),
                r.u64(b, base + 16).unwrap_or(0),
            )
        } else {
            (
                r.u32(b, base + 8).unwrap_or(0) as u64,
                r.u32(b, base + 12).unwrap_or(0) as u64,
            )
        };
        slices.push(Slice {
            architecture: macho_arch(cputype, cpusubtype),
            offset,
            size,
        });
    }
    if slices.is_empty() {
        return Err("malformed Mach-O universal binary: no readable architecture entries".to_string());
    }

    let chosen = match &opts.arch {
        Some(want) => {
            let w = want.trim().to_ascii_lowercase();
            slices
                .iter()
                .position(|s| s.architecture.to_ascii_lowercase() == w)
                .ok_or_else(|| {
                    let have: Vec<&str> =
                        slices.iter().map(|s| s.architecture.as_str()).collect();
                    format!(
                        "architecture {want:?} is not in this universal binary — it contains: {}",
                        have.join(", ")
                    )
                })?
        }
        None => 0,
    };
    let sel = &slices[chosen];
    let start = sel.offset as usize;
    if start >= b.len() {
        return Err(format!(
            "malformed Mach-O universal binary: slice {} starts at offset {} but the file is {} bytes",
            sel.architecture,
            sel.offset,
            b.len()
        ));
    }
    let name = sel.architecture.clone();
    let mut info = parse_macho(b, start, opts, &slices)?;
    info.selected_architecture = Some(name);
    Ok(info)
}

fn parse_macho(
    b: &[u8],
    start: usize,
    opts: &Options,
    slices: &[Slice],
) -> Result<BinaryInfo, String> {
    let magic = {
        let s = b
            .get(start..start + 4)
            .ok_or_else(|| "truncated Mach-O: no magic number at the slice offset".to_string())?;
        u32::from_le_bytes([s[0], s[1], s[2], s[3]])
    };
    let (bits, wide, le) = match magic {
        MH_MAGIC => (32u8, false, true),
        MH_MAGIC_64 => (64, true, true),
        MH_CIGAM => (32, false, false),
        MH_CIGAM_64 => (64, true, false),
        _ => {
            return Err(format!(
                "not a Mach-O slice: magic 0x{magic:08x} (expected feedface / feedfacf)"
            ))
        }
    };
    let r = Reader { le };
    let hdr_size = if wide { 32usize } else { 28 };
    if b.len() < start + hdr_size {
        return Err(format!(
            "truncated Mach-O: {} bytes available from the header, need {hdr_size}",
            b.len().saturating_sub(start)
        ));
    }
    let cputype = r.u32(b, start + 4).unwrap_or(0);
    let cpusubtype = r.u32(b, start + 8).unwrap_or(0);
    let filetype = r.u32(b, start + 12).unwrap_or(0);
    let ncmds = (r.u32(b, start + 16).unwrap_or(0) as usize).min(MAX_TABLE_ENTRIES);
    let mh_flags = r.u32(b, start + 24).unwrap_or(0);

    let mut sections: Vec<Section> = Vec::new();
    let mut libraries: Vec<Library> = Vec::new();
    let mut rpaths: Vec<String> = Vec::new();
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut imports: Vec<String> = Vec::new();
    let mut exports: Vec<String> = Vec::new();
    let mut linker: Option<String> = None;
    let mut install_name: Option<String> = None;
    let mut uuid: Option<String> = None;
    let mut platform: Option<String> = None;
    let mut entry_off: Option<u64> = None;
    let mut text_vmaddr: u64 = 0;
    let mut symtab: Option<(u32, u32, u32, u32)> = None; // symoff, nsyms, stroff, strsize

    let mut cursor = start + hdr_size;
    for _ in 0..ncmds {
        let Some(cmd) = r.u32(b, cursor) else { break };
        let Some(cmdsize) = r.u32(b, cursor + 4) else {
            break;
        };
        // A zero/absurd cmdsize would loop forever or walk off the slice.
        if cmdsize < 8 || cursor.saturating_add(cmdsize as usize) > b.len() {
            break;
        }
        // An `lc_str` is a byte offset from the START of its load command.
        let lc_str = |field: usize| -> String {
            let off = r.u32(b, cursor + field).unwrap_or(0) as usize;
            if off < 8 || off >= cmdsize as usize {
                return String::new();
            }
            cstr(b, cursor + off)
        };
        match cmd {
            0x1 | 0x19 => {
                // LC_SEGMENT / LC_SEGMENT_64
                let seg_wide = cmd == 0x19;
                let segname = b
                    .get(cursor + 8..cursor + 24)
                    .map(fixed_str)
                    .unwrap_or_default();
                let (vmaddr_o, nsects_o, sect_start, sect_size) = if seg_wide {
                    (24usize, 64usize, 72usize, 80usize)
                } else {
                    (24, 48, 56, 68)
                };
                let vmaddr = r.word(b, cursor + vmaddr_o, seg_wide).unwrap_or(0);
                if segname == "__TEXT" {
                    text_vmaddr = vmaddr;
                }
                let nsects = (r.u32(b, cursor + nsects_o).unwrap_or(0) as usize)
                    .min(MAX_TABLE_ENTRIES);
                for i in 0..nsects {
                    let sb = cursor + sect_start + i * sect_size;
                    if sb + sect_size > cursor + cmdsize as usize {
                        break;
                    }
                    let sectname = b.get(sb..sb + 16).map(fixed_str).unwrap_or_default();
                    let owner = b.get(sb + 16..sb + 32).map(fixed_str).unwrap_or_default();
                    let (addr_o, size_o, off_o, flags_o) = if seg_wide {
                        (32usize, 40usize, 48usize, 64usize)
                    } else {
                        (32, 36, 40, 56)
                    };
                    let addr = r.word(b, sb + addr_o, seg_wide).unwrap_or(0);
                    let size = r.word(b, sb + size_o, seg_wide).unwrap_or(0);
                    let offset = r.u32(b, sb + off_o).unwrap_or(0) as u64;
                    let sflags = r.u32(b, sb + flags_o).unwrap_or(0);
                    sections.push(Section {
                        name: format!("{owner},{sectname}"),
                        kind: macho_section_type(sflags & 0xff),
                        flags: macho_section_flags(sflags),
                        address: addr,
                        offset,
                        size,
                    });
                }
            }
            0x2 => {
                // LC_SYMTAB
                symtab = Some((
                    r.u32(b, cursor + 8).unwrap_or(0),
                    r.u32(b, cursor + 12).unwrap_or(0),
                    r.u32(b, cursor + 16).unwrap_or(0),
                    r.u32(b, cursor + 20).unwrap_or(0),
                ));
            }
            0xe => linker = Some(lc_str(8)),  // LC_LOAD_DYLINKER
            0xd => install_name = Some(lc_str(8)), // LC_ID_DYLIB
            0xc | 0x18 | 0x20 | 0x8000_0018 | 0x8000_001f | 0x8000_0023 => {
                let kind = match cmd {
                    0xc => "LOAD_DYLIB",
                    0x18 | 0x8000_0018 => "LOAD_WEAK_DYLIB",
                    0x20 => "LAZY_LOAD_DYLIB",
                    0x8000_001f => "REEXPORT_DYLIB",
                    _ => "LOAD_UPWARD_DYLIB",
                };
                libraries.push(Library {
                    name: lc_str(8),
                    kind,
                    current_version: r.u32(b, cursor + 16).map(macho_version),
                    compatibility_version: r.u32(b, cursor + 20).map(macho_version),
                });
            }
            0x8000_001c => rpaths.push(lc_str(8)), // LC_RPATH
            0x1b => {
                // LC_UUID
                if let Some(u) = b.get(cursor + 8..cursor + 24) {
                    uuid = Some(format!(
                        "{}-{}-{}-{}-{}",
                        hex(&u[0..4]),
                        hex(&u[4..6]),
                        hex(&u[6..8]),
                        hex(&u[8..10]),
                        hex(&u[10..16])
                    ));
                }
            }
            0x24 | 0x25 | 0x2f | 0x30 => {
                // LC_VERSION_MIN_{MACOSX,IPHONEOS,TVOS,WATCHOS}
                let os = match cmd {
                    0x24 => "macOS",
                    0x25 => "iOS",
                    0x2f => "tvOS",
                    _ => "watchOS",
                };
                let min = r.u32(b, cursor + 8).map(macho_version).unwrap_or_default();
                let sdk = r.u32(b, cursor + 12).map(macho_version).unwrap_or_default();
                platform = Some(format!("{os} {min} (SDK {sdk})"));
            }
            0x32 => {
                // LC_BUILD_VERSION
                let os = macho_platform(r.u32(b, cursor + 8).unwrap_or(0));
                let min = r.u32(b, cursor + 12).map(macho_version).unwrap_or_default();
                let sdk = r.u32(b, cursor + 16).map(macho_version).unwrap_or_default();
                platform = Some(format!("{os} {min} (SDK {sdk})"));
            }
            0x8000_0028 => entry_off = r.u64(b, cursor + 8), // LC_MAIN
            _ => {}
        }
        cursor += cmdsize as usize;
    }

    // --- symbol table ------------------------------------------------------
    // `symoff`/`stroff` are offsets into the Mach-O FILE, which inside a
    // universal archive means the slice — not the archive. Rebasing on `start`
    // is what makes a fat slice's symbol names readable instead of garbage.
    let mut has_symbols = false;
    if let Some((symoff, nsyms, stroff, _strsize)) = symtab {
        has_symbols = nsyms > 0;
        let symoff = start.saturating_add(symoff as usize);
        let stroff = start.saturating_add(stroff as usize);
        let ent = if wide { 16usize } else { 12 };
        let count = (nsyms as usize).min(MAX_TABLE_ENTRIES);
        for i in 0..count {
            let Some(base) = symoff.checked_add(i.saturating_mul(ent)) else {
                break;
            };
            let Some(strx) = r.u32(b, base) else { break };
            let n_type = *b.get(base + 4).unwrap_or(&0);
            let n_desc = r.u16(b, base + 6).unwrap_or(0);
            let value = if wide {
                r.u64(b, base + 8).unwrap_or(0)
            } else {
                r.u32(b, base + 8).unwrap_or(0) as u64
            };
            let name = cstr(b, stroff.saturating_add(strx as usize));
            let is_stab = n_type & 0xe0 != 0;
            let n_ext = n_type & 0x01 != 0;
            let n_pext = n_type & 0x10 != 0;
            let kind = if is_stab {
                "STAB"
            } else {
                match n_type & 0x0e {
                    0x0 => "UNDF",
                    0x2 => "ABS",
                    0xa => "INDR",
                    0xc => "PBUD",
                    0xe => "SECT",
                    _ => "unknown",
                }
            };
            let mut binding = if n_ext {
                "external"
            } else if n_pext {
                "private external"
            } else {
                "local"
            }
            .to_string();
            // N_WEAK_REF / N_WEAK_DEF live in n_desc.
            if n_desc & 0x0040 != 0 || n_desc & 0x0080 != 0 {
                binding.push_str(" (weak)");
            }
            if !is_stab && n_ext && !name.is_empty() {
                if kind == "UNDF" {
                    imports.push(name.clone());
                } else {
                    exports.push(name.clone());
                }
            }
            symbols.push(Symbol {
                name,
                kind: kind.to_string(),
                binding,
                value,
                size: 0, // nlist has no size field
                table: "LC_SYMTAB".to_string(),
            });
        }
    }

    let want_imports = opts.imports;
    Ok(BinaryInfo {
        format: "Mach-O",
        bits,
        endianness: if le { "little-endian" } else { "big-endian" },
        architecture: macho_arch(cputype, cpusubtype),
        file_type: macho_file_type(filetype).to_string(),
        os_abi: None,
        platform,
        entry_point: entry_off.map(|e| text_vmaddr.saturating_add(e)),
        is_pie: mh_flags & 0x0020_0000 != 0, // MH_PIE
        is_dynamic: mh_flags & 0x4 != 0 || !libraries.is_empty(), // MH_DYLDLINK
        is_stripped: !has_symbols,
        linker,
        install_name,
        uuid,
        flags: macho_header_flags(mh_flags),
        rpaths,
        libraries: if want_imports {
            Capped::build(libraries, opts.limit)
        } else {
            Capped::empty()
        },
        sections: if opts.sections {
            Capped::build(sections, opts.limit)
        } else {
            Capped::empty()
        },
        symbols: if opts.symbols {
            Capped::build(symbols, opts.limit)
        } else {
            Capped::empty()
        },
        imports: if want_imports {
            Capped::build(imports, opts.limit)
        } else {
            Capped::empty()
        },
        exports: if want_imports {
            Capped::build(exports, opts.limit)
        } else {
            Capped::empty()
        },
        architectures: slices.to_vec(),
        selected_architecture: None,
    })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Mach-O packs a version as `X.Y.Z` in `xxxx.yy.zz` nibble-pair form.
fn macho_version(v: u32) -> String {
    format!("{}.{}.{}", v >> 16, (v >> 8) & 0xff, v & 0xff)
}

const CPU_ARCH_ABI64: u32 = 0x0100_0000;
const CPU_ARCH_ABI64_32: u32 = 0x0200_0000;

fn macho_arch(cputype: u32, cpusubtype: u32) -> String {
    // The top byte of cpusubtype holds capability bits, not the model.
    let sub = cpusubtype & 0x00ff_ffff;
    match cputype {
        7 => "i386".to_string(),
        t if t == 7 | CPU_ARCH_ABI64 => match sub {
            8 => "x86_64h".to_string(),
            _ => "x86_64".to_string(),
        },
        12 => match sub {
            9 => "armv7".to_string(),
            11 => "armv7s".to_string(),
            12 => "armv7k".to_string(),
            6 => "armv6".to_string(),
            _ => "arm".to_string(),
        },
        t if t == 12 | CPU_ARCH_ABI64 => match sub {
            2 => "arm64e".to_string(),
            1 => "arm64v8".to_string(),
            _ => "arm64".to_string(),
        },
        t if t == 12 | CPU_ARCH_ABI64_32 => "arm64_32".to_string(),
        18 => "ppc".to_string(),
        t if t == 18 | CPU_ARCH_ABI64 => "ppc64".to_string(),
        6 => "m68k".to_string(),
        14 => "sparc".to_string(),
        _ => format!("unknown (cputype {cputype})"),
    }
}

fn macho_file_type(t: u32) -> &'static str {
    match t {
        1 => "Relocatable object",
        2 => "Executable",
        3 => "Fixed VM shared library",
        4 => "Core dump",
        5 => "Preloaded executable",
        6 => "Dynamic library (dylib)",
        7 => "Dynamic linker",
        8 => "Bundle",
        9 => "Dylib stub",
        10 => "Debug symbols (dSYM)",
        11 => "Kernel extension (kext)",
        12 => "Fileset",
        13 => "GPU program",
        14 => "GPU dylib",
        _ => "unknown",
    }
}

fn macho_platform(p: u32) -> &'static str {
    match p {
        1 => "macOS",
        2 => "iOS",
        3 => "tvOS",
        4 => "watchOS",
        5 => "bridgeOS",
        6 => "Mac Catalyst",
        7 => "iOS Simulator",
        8 => "tvOS Simulator",
        9 => "watchOS Simulator",
        10 => "DriverKit",
        11 => "visionOS",
        12 => "visionOS Simulator",
        _ => "unknown platform",
    }
}

fn macho_section_type(t: u32) -> String {
    let s = match t {
        0x0 => "REGULAR",
        0x1 => "ZEROFILL",
        0x2 => "CSTRING_LITERALS",
        0x3 => "4BYTE_LITERALS",
        0x4 => "8BYTE_LITERALS",
        0x5 => "LITERAL_POINTERS",
        0x6 => "NON_LAZY_SYMBOL_POINTERS",
        0x7 => "LAZY_SYMBOL_POINTERS",
        0x8 => "SYMBOL_STUBS",
        0x9 => "MOD_INIT_FUNC_POINTERS",
        0xa => "MOD_TERM_FUNC_POINTERS",
        0xb => "COALESCED",
        0xc => "GB_ZEROFILL",
        0xd => "INTERPOSING",
        0xe => "16BYTE_LITERALS",
        0x11 => "THREAD_LOCAL_REGULAR",
        0x12 => "THREAD_LOCAL_ZEROFILL",
        0x13 => "THREAD_LOCAL_VARIABLES",
        0x16 => "INIT_FUNC_OFFSETS",
        _ => "",
    };
    if s.is_empty() {
        format!("0x{t:x}")
    } else {
        s.to_string()
    }
}

fn macho_section_flags(f: u32) -> Vec<String> {
    const NAMES: [(u32, &str); 7] = [
        (0x8000_0000, "pure-instructions"),
        (0x4000_0000, "no-toc"),
        (0x2000_0000, "strip-static-syms"),
        (0x1000_0000, "no-dead-strip"),
        (0x0800_0000, "live-support"),
        (0x0400_0000, "self-modifying-code"),
        (0x0000_0400, "some-instructions"),
    ];
    NAMES
        .iter()
        .filter(|(bit, _)| f & bit != 0)
        .map(|(_, n)| n.to_string())
        .collect()
}

fn macho_header_flags(f: u32) -> Vec<String> {
    const NAMES: [(u32, &str); 14] = [
        (0x1, "NOUNDEFS"),
        (0x4, "DYLDLINK"),
        (0x8, "BINDATLOAD"),
        (0x10, "PREBOUND"),
        (0x20, "SPLIT_SEGS"),
        (0x80, "TWOLEVEL"),
        (0x100, "FORCE_FLAT"),
        (0x2000, "SUBSECTIONS_VIA_SYMBOLS"),
        (0x8000, "WEAK_DEFINES"),
        (0x1_0000, "BINDS_TO_WEAK"),
        (0x2_0000, "ALLOW_STACK_EXECUTION"),
        (0x20_0000, "PIE"),
        (0x100_0000, "NO_HEAP_EXECUTION"),
        (0x200_0000, "APP_EXTENSION_SAFE"),
    ];
    NAMES
        .iter()
        .filter(|(bit, _)| f & bit != 0)
        .map(|(_, n)| n.to_string())
        .collect()
}

#[cfg(test)]
mod tests;

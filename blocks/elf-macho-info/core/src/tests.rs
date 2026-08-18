//! Tests for the ELF + Mach-O parser.
//!
//! The fixtures are BUILT BYTE BY BYTE rather than checked in: a real `a.out`
//! would be a multi-megabyte, platform-specific blob, whereas a hand-laid
//! header + section/load-command table exercises exactly the fields the parser
//! reads and lets a test say "this offset means that". One test does go through
//! a genuinely real binary — the test executable itself via `/proc/self/exe` —
//! so the synthetic fixtures can't drift into a private dialect the parser
//! agrees with and `readelf` doesn't.

use super::*;

// ---------------------------------------------------------------------------
// Little helpers for laying bytes into a buffer
// ---------------------------------------------------------------------------

/// Write `src` at `off`, growing the buffer as needed. Fixtures are described
/// as "field X lives at offset Y", so an absolute-position writer keeps the
/// test readable next to the spec.
fn put(buf: &mut Vec<u8>, off: usize, src: &[u8]) {
    if buf.len() < off + src.len() {
        buf.resize(off + src.len(), 0);
    }
    buf[off..off + src.len()].copy_from_slice(src);
}

fn put_u16(buf: &mut Vec<u8>, off: usize, v: u16) {
    put(buf, off, &v.to_le_bytes());
}
fn put_u32(buf: &mut Vec<u8>, off: usize, v: u32) {
    put(buf, off, &v.to_le_bytes());
}
fn put_u64(buf: &mut Vec<u8>, off: usize, v: u64) {
    put(buf, off, &v.to_le_bytes());
}
fn put_u16be(buf: &mut Vec<u8>, off: usize, v: u16) {
    put(buf, off, &v.to_be_bytes());
}
fn put_u32be(buf: &mut Vec<u8>, off: usize, v: u32) {
    put(buf, off, &v.to_be_bytes());
}

/// A NUL-separated string table plus the offsets it handed out — the shape both
/// containers use for every name in the file.
#[derive(Default)]
struct StrTab {
    bytes: Vec<u8>,
}

impl StrTab {
    /// A real string table starts with the empty string at index 0, so offset 0
    /// reads back as "no name".
    fn new() -> Self {
        StrTab { bytes: vec![0] }
    }
    fn add(&mut self, s: &str) -> u32 {
        let off = self.bytes.len() as u32;
        self.bytes.extend_from_slice(s.as_bytes());
        self.bytes.push(0);
        off
    }
}

// ---------------------------------------------------------------------------
// ELF64 fixture
// ---------------------------------------------------------------------------

// Fixed layout, chosen so every table is comfortably aligned and no two
// overlap. Kept as constants because the section headers, program headers and
// the assertions all have to agree on them.
const E64_PHOFF: usize = 0x40;
const E64_INTERP: usize = 0x0e8;
const E64_TEXT: usize = 0x110;
const E64_DYNSYM: usize = 0x120;
const E64_DYNSTR: usize = 0x180;
const E64_DYNAMIC: usize = 0x200;
const E64_SYMTAB: usize = 0x300;
const E64_STRTAB: usize = 0x380;
const E64_SHSTRTAB: usize = 0x400;
const E64_SHOFF: usize = 0x500;

const INTERP: &str = "/lib64/ld-linux-x86-64.so.2";

/// One `Elf64_Shdr`.
fn elf64_shdr(
    buf: &mut Vec<u8>,
    idx: usize,
    name: u32,
    sh_type: u32,
    flags: u64,
    addr: u64,
    offset: u64,
    size: u64,
    link: u32,
    entsize: u64,
) {
    let b = E64_SHOFF + idx * 64;
    put_u32(buf, b, name);
    put_u32(buf, b + 4, sh_type);
    put_u64(buf, b + 8, flags);
    put_u64(buf, b + 16, addr);
    put_u64(buf, b + 24, offset);
    put_u64(buf, b + 32, size);
    put_u32(buf, b + 40, link);
    put_u64(buf, b + 56, entsize);
}

/// One `Elf64_Sym`.
fn elf64_sym(buf: &mut Vec<u8>, at: usize, name: u32, info: u8, shndx: u16, value: u64, size: u64) {
    put_u32(buf, at, name);
    put(buf, at + 4, &[info, 0]);
    put_u16(buf, at + 6, shndx);
    put_u64(buf, at + 8, value);
    put_u64(buf, at + 16, size);
}

/// A complete, self-consistent little-endian ELF64 shared object: three program
/// headers (LOAD/DYNAMIC/INTERP), eight sections, a dynamic symbol table with
/// one import and one export, a full `.symtab`, and a `.dynamic` carrying
/// `DT_NEEDED` × 2, `DT_SONAME` and `DT_RUNPATH`.
fn elf64_fixture() -> Vec<u8> {
    let mut b: Vec<u8> = vec![0; 0x700];

    // --- e_ident + file header --------------------------------------------
    put(&mut b, 0, b"\x7fELF");
    b[4] = 2; // ELFCLASS64
    b[5] = 1; // ELFDATA2LSB
    b[6] = 1; // EV_CURRENT
    b[7] = 3; // ELFOSABI_LINUX
    put_u16(&mut b, 16, 3); // ET_DYN
    put_u16(&mut b, 18, 62); // EM_X86_64
    put_u32(&mut b, 20, 1);
    put_u64(&mut b, 24, 0x1040); // e_entry
    put_u64(&mut b, 32, E64_PHOFF as u64);
    put_u64(&mut b, 40, E64_SHOFF as u64);
    put_u32(&mut b, 48, 0); // e_flags
    put_u16(&mut b, 52, 64); // e_ehsize
    put_u16(&mut b, 54, 56); // e_phentsize
    put_u16(&mut b, 56, 3); // e_phnum
    put_u16(&mut b, 58, 64); // e_shentsize
    put_u16(&mut b, 60, 8); // e_shnum
    put_u16(&mut b, 62, 7); // e_shstrndx -> .shstrtab

    // --- string tables -----------------------------------------------------
    let mut dynstr = StrTab::new();
    let s_libc = dynstr.add("libc.so.6");
    let s_libm = dynstr.add("libm.so.6");
    let s_soname = dynstr.add("libtest.so");
    let s_runpath = dynstr.add("/opt/lib:/usr/local/lib");
    let s_puts = dynstr.add("puts");
    let s_myfunc = dynstr.add("my_func");
    let s_local = dynstr.add("hidden_helper");

    let mut strtab = StrTab::new();
    let t_main = strtab.add("main");
    let t_static = strtab.add("static_helper");

    let mut shstr = StrTab::new();
    let n_text = shstr.add(".text");
    let n_dynsym = shstr.add(".dynsym");
    let n_dynstr = shstr.add(".dynstr");
    let n_dynamic = shstr.add(".dynamic");
    let n_symtab = shstr.add(".symtab");
    let n_strtab = shstr.add(".strtab");
    let n_shstrtab = shstr.add(".shstrtab");

    // --- program headers (Elf64_Phdr, 56 bytes each) -----------------------
    // PT_LOAD covering the whole file, mapped 1:1 so vaddr == file offset.
    let p = E64_PHOFF;
    put_u32(&mut b, p, 1); // PT_LOAD
    put_u32(&mut b, p + 4, 5); // PF_R | PF_X
    put_u64(&mut b, p + 8, 0); // p_offset
    put_u64(&mut b, p + 16, 0); // p_vaddr
    put_u64(&mut b, p + 32, 0x700); // p_filesz
    put_u64(&mut b, p + 40, 0x700); // p_memsz

    let dyn_size = 6 * 16u64; // 5 entries + DT_NULL
    let p = E64_PHOFF + 56;
    put_u32(&mut b, p, 2); // PT_DYNAMIC
    put_u64(&mut b, p + 8, E64_DYNAMIC as u64);
    put_u64(&mut b, p + 16, E64_DYNAMIC as u64);
    put_u64(&mut b, p + 32, dyn_size);

    let p = E64_PHOFF + 112;
    put_u32(&mut b, p, 3); // PT_INTERP
    put_u64(&mut b, p + 8, E64_INTERP as u64);
    put_u64(&mut b, p + 16, E64_INTERP as u64);
    put_u64(&mut b, p + 32, INTERP.len() as u64 + 1);

    put(&mut b, E64_INTERP, INTERP.as_bytes());
    put(&mut b, E64_INTERP + INTERP.len(), &[0]);

    // --- .dynsym: null, one undefined import, one defined export -----------
    elf64_sym(&mut b, E64_DYNSYM, 0, 0, 0, 0, 0);
    // (GLOBAL << 4) | FUNC, SHN_UNDEF -> an import.
    elf64_sym(&mut b, E64_DYNSYM + 24, s_puts, 0x12, 0, 0, 0);
    // (GLOBAL << 4) | FUNC, defined in .text -> an export.
    elf64_sym(&mut b, E64_DYNSYM + 48, s_myfunc, 0x12, 1, 0x1150, 0x30);
    // (LOCAL << 4) | FUNC — a local dynsym entry is neither import nor export.
    elf64_sym(&mut b, E64_DYNSYM + 72, s_local, 0x02, 1, 0x1180, 0x10);

    put(&mut b, E64_DYNSTR, &dynstr.bytes);

    // --- .dynamic ----------------------------------------------------------
    let d = E64_DYNAMIC;
    let entries: [(u64, u64); 6] = [
        (1, s_libc as u64),                // DT_NEEDED
        (1, s_libm as u64),                // DT_NEEDED
        (14, s_soname as u64),             // DT_SONAME
        (29, s_runpath as u64),            // DT_RUNPATH
        (5, E64_DYNSTR as u64),            // DT_STRTAB (vaddr == offset here)
        (0, 0),                            // DT_NULL
    ];
    for (i, (tag, val)) in entries.iter().enumerate() {
        put_u64(&mut b, d + i * 16, *tag);
        put_u64(&mut b, d + i * 16 + 8, *val);
    }

    // --- .symtab -----------------------------------------------------------
    elf64_sym(&mut b, E64_SYMTAB, 0, 0, 0, 0, 0);
    elf64_sym(&mut b, E64_SYMTAB + 24, t_main, 0x12, 1, 0x1140, 0x20);
    // (LOCAL << 4) | FUNC
    elf64_sym(&mut b, E64_SYMTAB + 48, t_static, 0x02, 1, 0x1100, 0x10);

    put(&mut b, E64_STRTAB, &strtab.bytes);
    put(&mut b, E64_SHSTRTAB, &shstr.bytes);

    // --- section headers ---------------------------------------------------
    elf64_shdr(&mut b, 0, 0, 0, 0, 0, 0, 0, 0, 0); // SHT_NULL
    // .text: PROGBITS, ALLOC|EXECINSTR
    elf64_shdr(&mut b, 1, n_text, 1, 0x2 | 0x4, 0x1040, E64_TEXT as u64, 0x10, 0, 0);
    // .dynsym -> strings in section 3
    elf64_shdr(&mut b, 2, n_dynsym, 11, 0x2, 0x1120, E64_DYNSYM as u64, 96, 3, 24);
    elf64_shdr(
        &mut b,
        3,
        n_dynstr,
        3,
        0x2,
        E64_DYNSTR as u64,
        E64_DYNSTR as u64,
        dynstr.bytes.len() as u64,
        0,
        0,
    );
    elf64_shdr(
        &mut b,
        4,
        n_dynamic,
        6,
        0x2 | 0x1,
        E64_DYNAMIC as u64,
        E64_DYNAMIC as u64,
        dyn_size,
        3,
        16,
    );
    // .symtab -> strings in section 6
    elf64_shdr(&mut b, 5, n_symtab, 2, 0, 0, E64_SYMTAB as u64, 72, 6, 24);
    elf64_shdr(
        &mut b,
        6,
        n_strtab,
        3,
        0,
        0,
        E64_STRTAB as u64,
        strtab.bytes.len() as u64,
        0,
        0,
    );
    elf64_shdr(
        &mut b,
        7,
        n_shstrtab,
        3,
        0,
        0,
        E64_SHSTRTAB as u64,
        shstr.bytes.len() as u64,
        0,
        0,
    );

    b
}

// ---------------------------------------------------------------------------
// ELF happy paths
// ---------------------------------------------------------------------------

#[test]
fn elf64_header_is_decoded() {
    let info = parse(&elf64_fixture(), &Options::default()).expect("parses");
    assert_eq!(info.format, "ELF");
    assert_eq!(info.bits, 64);
    assert_eq!(info.endianness, "little-endian");
    assert_eq!(info.architecture, "x86-64");
    assert_eq!(info.file_type, "Shared object / PIE");
    assert_eq!(info.os_abi.as_deref(), Some("Linux (GNU)"));
    assert_eq!(info.platform, None, "platform is a Mach-O-only concept");
    assert_eq!(info.entry_point, Some(0x1040));
    assert!(info.is_pie, "ET_DYN is position-independent");
    assert!(info.is_dynamic, "PT_DYNAMIC present");
    assert!(!info.is_stripped, ".symtab present");
    assert_eq!(info.linker.as_deref(), Some(INTERP));
    assert_eq!(info.uuid, None, "UUID is a Mach-O-only concept");
    assert!(info.architectures.is_empty(), "an ELF has no fat slices");
}

#[test]
fn elf64_sections_carry_names_types_and_flags() {
    let info = parse(&elf64_fixture(), &Options::default()).expect("parses");
    assert_eq!(info.sections.total, 8);
    assert!(!info.sections.truncated);

    let names: Vec<&str> = info.sections.items.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "", ".text", ".dynsym", ".dynstr", ".dynamic", ".symtab", ".strtab", ".shstrtab"
        ]
    );

    let text = &info.sections.items[1];
    assert_eq!(text.kind, "PROGBITS");
    assert_eq!(text.flags, vec!["alloc".to_string(), "execinstr".to_string()]);
    assert_eq!(text.address, 0x1040);
    assert_eq!(text.offset, E64_TEXT as u64);
    assert_eq!(text.size, 0x10);

    assert_eq!(info.sections.items[2].kind, "DYNSYM");
    assert_eq!(info.sections.items[4].kind, "DYNAMIC");
}

#[test]
fn elf64_nobits_section_reports_no_file_offset() {
    // A .bss occupies memory but no file bytes; reporting its sh_offset would
    // point a reader at unrelated data.
    let mut b = elf64_fixture();
    put_u32(&mut b, E64_SHOFF + 64 + 4, 8); // section 1 -> SHT_NOBITS
    let info = parse(&b, &Options::default()).expect("parses");
    let s = &info.sections.items[1];
    assert_eq!(s.kind, "NOBITS");
    assert_eq!(s.offset, 0, "NOBITS has no bytes in the file");
    assert_eq!(s.address, 0x1040, "but it still has an address");
}

#[test]
fn elf64_symbols_come_from_both_tables_with_type_and_binding() {
    let info = parse(&elf64_fixture(), &Options::default()).expect("parses");
    // 4 .dynsym + 3 .symtab entries, null symbols included.
    assert_eq!(info.symbols.total, 7);

    let main = info
        .symbols
        .items
        .iter()
        .find(|s| s.name == "main")
        .expect("main");
    assert_eq!(main.kind, "FUNC");
    assert_eq!(main.binding, "GLOBAL");
    assert_eq!(main.value, 0x1140);
    assert_eq!(main.size, 0x20);
    assert_eq!(main.table, ".symtab");

    let helper = info
        .symbols
        .items
        .iter()
        .find(|s| s.name == "static_helper")
        .expect("static_helper");
    assert_eq!(helper.binding, "LOCAL");

    let puts = info
        .symbols
        .items
        .iter()
        .find(|s| s.name == "puts")
        .expect("puts");
    assert_eq!(puts.table, ".dynsym");
    assert_eq!(puts.value, 0, "an undefined symbol has no address");
}

#[test]
fn elf64_splits_dynamic_symbols_into_imports_and_exports() {
    let info = parse(&elf64_fixture(), &Options::default()).expect("parses");
    assert_eq!(info.imports.items, vec!["puts".to_string()]);
    assert_eq!(info.exports.items, vec!["my_func".to_string()]);
    assert!(
        !info.exports.items.contains(&"hidden_helper".to_string()),
        "a LOCAL dynsym entry is neither imported nor exported"
    );
}

#[test]
fn elf64_reads_needed_libraries_soname_and_runpath() {
    let info = parse(&elf64_fixture(), &Options::default()).expect("parses");
    let libs: Vec<&str> = info.libraries.items.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(libs, ["libc.so.6", "libm.so.6"]);
    assert!(info.libraries.items.iter().all(|l| l.kind == "NEEDED"));
    assert!(
        info.libraries.items[0].current_version.is_none(),
        "versioned dylibs are a Mach-O concept"
    );
    assert_eq!(info.install_name.as_deref(), Some("libtest.so"));
    // DT_RUNPATH is one colon-separated string; a reader wants the entries.
    assert_eq!(
        info.rpaths,
        vec!["/opt/lib".to_string(), "/usr/local/lib".to_string()]
    );
}

#[test]
fn elf64_falls_back_to_pt_dynamic_when_sections_are_stripped() {
    // A section-header-stripped binary still has PT_DYNAMIC + DT_STRTAB, which
    // is how the linked libraries survive `strip --strip-all`.
    let mut b = elf64_fixture();
    put_u64(&mut b, 40, 0); // e_shoff = 0
    put_u16(&mut b, 60, 0); // e_shnum = 0
    put_u16(&mut b, 62, 0); // e_shstrndx = 0

    let info = parse(&b, &Options::default()).expect("parses");
    assert!(info.sections.items.is_empty());
    assert!(info.symbols.items.is_empty());
    assert!(info.is_stripped, "no SHT_SYMTAB left");
    assert!(info.is_dynamic);
    let libs: Vec<&str> = info.libraries.items.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        libs,
        ["libc.so.6", "libm.so.6"],
        "DT_STRTAB resolved through PT_LOAD"
    );
    assert_eq!(info.install_name.as_deref(), Some("libtest.so"));
}

#[test]
fn elf64_static_executable_is_not_dynamic() {
    let mut b = elf64_fixture();
    put_u16(&mut b, 16, 2); // ET_EXEC
    put_u16(&mut b, 56, 1); // keep only PT_LOAD
    put_u32(&mut b, E64_SHOFF + 4 * 64 + 4, 1); // .dynamic -> PROGBITS

    let info = parse(&b, &Options::default()).expect("parses");
    assert_eq!(info.file_type, "Executable");
    assert!(!info.is_pie);
    assert!(!info.is_dynamic);
    assert!(info.libraries.items.is_empty());
    assert_eq!(info.linker, None, "no PT_INTERP");
}

#[test]
fn elf32_big_endian_powerpc_parses() {
    // The 32-bit big-endian path shares no offsets with ELF64, so it gets its
    // own end-to-end fixture rather than a tweak of the 64-bit one.
    let mut b: Vec<u8> = vec![0; 0x200];
    put(&mut b, 0, b"\x7fELF");
    b[4] = 1; // ELFCLASS32
    b[5] = 2; // ELFDATA2MSB
    b[6] = 1;
    b[7] = 0; // System V
    put_u16be(&mut b, 16, 2); // ET_EXEC
    put_u16be(&mut b, 18, 20); // EM_PPC
    put_u32be(&mut b, 24, 0x1000_0100); // e_entry
    put_u32be(&mut b, 32, 0); // e_shoff
    put_u16be(&mut b, 44, 0); // e_phnum
    put_u16be(&mut b, 46, 40); // e_shentsize
    put_u16be(&mut b, 48, 0); // e_shnum

    let info = parse(&b, &Options::default()).expect("parses");
    assert_eq!(info.bits, 32);
    assert_eq!(info.endianness, "big-endian");
    assert_eq!(info.architecture, "PowerPC");
    assert_eq!(info.file_type, "Executable");
    assert_eq!(info.os_abi.as_deref(), Some("System V"));
    assert_eq!(info.entry_point, Some(0x1000_0100));
    assert!(info.is_stripped);
}

#[test]
fn elf_arm_header_flags_are_decoded() {
    let mut b = elf64_fixture();
    put_u16(&mut b, 18, 40); // EM_ARM
    put_u32(&mut b, 48, 0x0500_0400); // EABI v5 + hard-float
    let info = parse(&b, &Options::default()).expect("parses");
    assert_eq!(info.architecture, "ARM");
    assert!(info.flags.contains(&"hard-float ABI".to_string()));
    assert!(info.flags.contains(&"EABI v5".to_string()));
}

#[test]
fn elf_unknown_machine_and_section_type_degrade_to_readable_labels() {
    let mut b = elf64_fixture();
    put_u16(&mut b, 18, 0x7abc); // no such EM_*
    put_u32(&mut b, E64_SHOFF + 64 + 4, 0x6000_0001); // OS-specific sh_type
    let info = parse(&b, &Options::default()).expect("parses");
    assert_eq!(info.architecture, "unknown (e_machine 31420)");
    assert_eq!(info.sections.items[1].kind, "0x60000001");
}

// ---------------------------------------------------------------------------
// Mach-O fixtures
// ---------------------------------------------------------------------------

const CPU_X86_64: u32 = 7 | 0x0100_0000;
const CPU_ARM64: u32 = 12 | 0x0100_0000;

/// A complete thin 64-bit little-endian Mach-O with a `__TEXT` segment (one
/// `__text` section), `LC_SYMTAB` (one export + one import), `LC_LOAD_DYLINKER`,
/// `LC_LOAD_DYLIB`, `LC_RPATH`, `LC_UUID`, `LC_BUILD_VERSION` and `LC_MAIN`.
fn macho64_fixture(cputype: u32, filetype: u32) -> Vec<u8> {
    let mut cmds: Vec<u8> = Vec::new();
    let mut ncmds = 0u32;

    // --- LC_SEGMENT_64 __TEXT with one __text section ---------------------
    let mut seg = vec![0u8; 72 + 80];
    put_u32(&mut seg, 0, 0x19); // LC_SEGMENT_64
    put_u32(&mut seg, 4, (72 + 80) as u32);
    put(&mut seg, 8, b"__TEXT");
    put_u64(&mut seg, 24, 0x1_0000_0000); // vmaddr
    put_u64(&mut seg, 32, 0x4000); // vmsize
    put_u64(&mut seg, 40, 0); // fileoff
    put_u64(&mut seg, 48, 0x4000); // filesize
    put_u32(&mut seg, 64, 1); // nsects
    let s = 72;
    put(&mut seg, s, b"__text");
    put(&mut seg, s + 16, b"__TEXT");
    put_u64(&mut seg, s + 32, 0x1_0000_3f00); // addr
    put_u64(&mut seg, s + 40, 0x120); // size
    put_u32(&mut seg, s + 48, 0x3f00); // offset
    put_u32(&mut seg, s + 64, 0x8000_0400); // pure-instructions|some-instructions
    cmds.extend_from_slice(&seg);
    ncmds += 1;

    // --- LC_SYMTAB (offsets patched once the tables are placed) -----------
    let symtab_cmd_at = cmds.len();
    let mut st = vec![0u8; 24];
    put_u32(&mut st, 0, 0x2);
    put_u32(&mut st, 4, 24);
    put_u32(&mut st, 12, 2); // nsyms
    cmds.extend_from_slice(&st);
    ncmds += 1;

    // --- LC_LOAD_DYLINKER -------------------------------------------------
    let dyld = "/usr/lib/dyld";
    let size = ((12 + dyld.len() + 1 + 7) / 8) * 8;
    let mut lc = vec![0u8; size];
    put_u32(&mut lc, 0, 0xe);
    put_u32(&mut lc, 4, size as u32);
    put_u32(&mut lc, 8, 12); // name offset within the command
    put(&mut lc, 12, dyld.as_bytes());
    cmds.extend_from_slice(&lc);
    ncmds += 1;

    // --- LC_LOAD_DYLIB ----------------------------------------------------
    let lib = "/usr/lib/libSystem.B.dylib";
    let size = ((24 + lib.len() + 1 + 7) / 8) * 8;
    let mut lc = vec![0u8; size];
    put_u32(&mut lc, 0, 0xc);
    put_u32(&mut lc, 4, size as u32);
    put_u32(&mut lc, 8, 24); // name offset
    put_u32(&mut lc, 12, 2); // timestamp
    put_u32(&mut lc, 16, (1319 << 16) | (0 << 8) | 0); // current_version 1319.0.0
    put_u32(&mut lc, 20, (1 << 16) | (2 << 8) | 3); // compat 1.2.3
    put(&mut lc, 24, lib.as_bytes());
    cmds.extend_from_slice(&lc);
    ncmds += 1;

    // --- LC_RPATH ---------------------------------------------------------
    let rpath = "@executable_path/../Frameworks";
    let size = ((12 + rpath.len() + 1 + 7) / 8) * 8;
    let mut lc = vec![0u8; size];
    put_u32(&mut lc, 0, 0x8000_001c);
    put_u32(&mut lc, 4, size as u32);
    put_u32(&mut lc, 8, 12);
    put(&mut lc, 12, rpath.as_bytes());
    cmds.extend_from_slice(&lc);
    ncmds += 1;

    // --- LC_UUID ----------------------------------------------------------
    let mut lc = vec![0u8; 24];
    put_u32(&mut lc, 0, 0x1b);
    put_u32(&mut lc, 4, 24);
    put(
        &mut lc,
        8,
        &[
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ],
    );
    cmds.extend_from_slice(&lc);
    ncmds += 1;

    // --- LC_BUILD_VERSION (macOS 14.0.0, SDK 15.0.0) ----------------------
    let mut lc = vec![0u8; 24];
    put_u32(&mut lc, 0, 0x32);
    put_u32(&mut lc, 4, 24);
    put_u32(&mut lc, 8, 1); // PLATFORM_MACOS
    put_u32(&mut lc, 12, 14 << 16);
    put_u32(&mut lc, 16, 15 << 16);
    cmds.extend_from_slice(&lc);
    ncmds += 1;

    // --- LC_MAIN ----------------------------------------------------------
    let mut lc = vec![0u8; 24];
    put_u32(&mut lc, 0, 0x8000_0028);
    put_u32(&mut lc, 4, 24);
    put_u64(&mut lc, 8, 0x3f00); // entryoff, relative to __TEXT vmaddr
    cmds.extend_from_slice(&lc);
    ncmds += 1;

    // --- assemble ----------------------------------------------------------
    let hdr = 32usize;
    let mut b: Vec<u8> = vec![0; hdr];
    put_u32(&mut b, 0, 0xfeed_facf); // MH_MAGIC_64
    put_u32(&mut b, 4, cputype);
    put_u32(&mut b, 8, 0); // cpusubtype: the generic model
    put_u32(&mut b, 12, filetype);
    put_u32(&mut b, 16, ncmds);
    put_u32(&mut b, 20, cmds.len() as u32);
    // MH_NOUNDEFS is deliberately off; DYLDLINK | TWOLEVEL | PIE.
    put_u32(&mut b, 24, 0x4 | 0x80 | 0x0020_0000);

    // Symbol + string tables sit after the load commands.
    let symoff = hdr + cmds.len();
    let stroff = symoff + 2 * 16;
    let mut strs = StrTab::new();
    let n_main = strs.add("_main");
    let n_printf = strs.add("_printf");

    put_u32(&mut cmds, symtab_cmd_at + 8, symoff as u32);
    put_u32(&mut cmds, symtab_cmd_at + 16, stroff as u32);
    put_u32(&mut cmds, symtab_cmd_at + 20, strs.bytes.len() as u32);
    b.extend_from_slice(&cmds);

    // nlist_64: n_strx u32, n_type u8, n_sect u8, n_desc u16, n_value u64.
    let mut nl = vec![0u8; 32];
    put_u32(&mut nl, 0, n_main);
    nl[4] = 0x0f; // N_SECT | N_EXT -> a defined external symbol
    nl[5] = 1;
    put_u64(&mut nl, 8, 0x1_0000_3f00);
    put_u32(&mut nl, 16, n_printf);
    nl[20] = 0x01; // N_UNDF | N_EXT -> an import
    put_u16(&mut nl, 22, 0x0040); // N_WEAK_REF
    b.extend_from_slice(&nl);
    b.extend_from_slice(&strs.bytes);
    b
}

#[test]
fn macho64_header_and_load_commands_are_decoded() {
    let info = parse(&macho64_fixture(CPU_ARM64, 2), &Options::default()).expect("parses");
    assert_eq!(info.format, "Mach-O");
    assert_eq!(info.bits, 64);
    assert_eq!(info.endianness, "little-endian");
    assert_eq!(info.architecture, "arm64");
    assert_eq!(info.file_type, "Executable");
    assert_eq!(info.os_abi, None, "OS/ABI is an ELF-only concept");
    assert_eq!(info.platform.as_deref(), Some("macOS 14.0.0 (SDK 15.0.0)"));
    assert_eq!(
        info.uuid.as_deref(),
        Some("00010203-0405-0607-0809-0a0b0c0d0e0f")
    );
    assert_eq!(info.linker.as_deref(), Some("/usr/lib/dyld"));
    assert_eq!(
        info.rpaths,
        vec!["@executable_path/../Frameworks".to_string()]
    );
    assert!(info.is_pie, "MH_PIE");
    assert!(info.is_dynamic, "MH_DYLDLINK");
    assert!(!info.is_stripped, "LC_SYMTAB has entries");
    // LC_MAIN's entryoff is relative to the __TEXT segment's vmaddr.
    assert_eq!(info.entry_point, Some(0x1_0000_3f00));
    assert_eq!(
        info.flags,
        vec![
            "DYLDLINK".to_string(),
            "TWOLEVEL".to_string(),
            "PIE".to_string()
        ]
    );
    assert!(info.architectures.is_empty(), "a thin binary has no slices");
}

#[test]
fn macho64_sections_are_named_segment_comma_section() {
    let info = parse(&macho64_fixture(CPU_X86_64, 2), &Options::default()).expect("parses");
    assert_eq!(info.sections.total, 1);
    let s = &info.sections.items[0];
    assert_eq!(s.name, "__TEXT,__text");
    assert_eq!(s.kind, "REGULAR");
    assert_eq!(
        s.flags,
        vec!["pure-instructions".to_string(), "some-instructions".to_string()]
    );
    assert_eq!(s.address, 0x1_0000_3f00);
    assert_eq!(s.offset, 0x3f00);
    assert_eq!(s.size, 0x120);
    assert_eq!(info.architecture, "x86_64");
}

#[test]
fn macho64_libraries_carry_current_and_compat_versions() {
    let info = parse(&macho64_fixture(CPU_ARM64, 2), &Options::default()).expect("parses");
    assert_eq!(info.libraries.total, 1);
    let l = &info.libraries.items[0];
    assert_eq!(l.name, "/usr/lib/libSystem.B.dylib");
    assert_eq!(l.kind, "LOAD_DYLIB");
    assert_eq!(l.current_version.as_deref(), Some("1319.0.0"));
    assert_eq!(l.compatibility_version.as_deref(), Some("1.2.3"));
}

#[test]
fn macho64_symbols_split_into_imports_and_exports() {
    let info = parse(&macho64_fixture(CPU_ARM64, 2), &Options::default()).expect("parses");
    assert_eq!(info.symbols.total, 2);

    let main = &info.symbols.items[0];
    assert_eq!(main.name, "_main");
    assert_eq!(main.kind, "SECT");
    assert_eq!(main.binding, "external");
    assert_eq!(main.value, 0x1_0000_3f00);
    assert_eq!(main.size, 0, "an nlist entry carries no size");
    assert_eq!(main.table, "LC_SYMTAB");

    let printf = &info.symbols.items[1];
    assert_eq!(printf.name, "_printf");
    assert_eq!(printf.kind, "UNDF");
    assert_eq!(
        printf.binding, "external (weak)",
        "N_WEAK_REF must be visible — it changes link behavior"
    );

    assert_eq!(info.imports.items, vec!["_printf".to_string()]);
    assert_eq!(info.exports.items, vec!["_main".to_string()]);
}

#[test]
fn macho_dylib_file_type_is_named() {
    let info = parse(&macho64_fixture(CPU_ARM64, 6), &Options::default()).expect("parses");
    assert_eq!(info.file_type, "Dynamic library (dylib)");
}

/// Wrap thin slices into a big-endian `cafebabe` universal archive.
fn fat_fixture(slices: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let hdr = 8 + slices.len() * 20;
    // Real fat binaries page-align each slice; 0x1000 keeps the offsets
    // realistic and catches any code that assumes a slice starts at 0.
    let mut offsets = Vec::new();
    let mut cur = hdr.div_ceil(0x1000) * 0x1000;
    for (_, body) in slices {
        offsets.push(cur);
        cur += body.len().div_ceil(0x1000) * 0x1000;
    }

    let mut b: Vec<u8> = vec![0; cur];
    put_u32be(&mut b, 0, 0xcafe_babe);
    put_u32be(&mut b, 4, slices.len() as u32);
    for (i, (cputype, body)) in slices.iter().enumerate() {
        let e = 8 + i * 20;
        put_u32be(&mut b, e, *cputype);
        put_u32be(&mut b, e + 4, 0); // cpusubtype
        put_u32be(&mut b, e + 8, offsets[i] as u32);
        put_u32be(&mut b, e + 12, body.len() as u32);
        put_u32be(&mut b, e + 16, 12); // align 2^12
        put(&mut b, offsets[i], body);
    }
    b
}

#[test]
fn fat_binary_lists_slices_and_reports_the_first_by_default() {
    let b = fat_fixture(&[
        (CPU_X86_64, macho64_fixture(CPU_X86_64, 2)),
        (CPU_ARM64, macho64_fixture(CPU_ARM64, 2)),
    ]);
    let info = parse(&b, &Options::default()).expect("parses");

    let arches: Vec<&str> = info
        .architectures
        .iter()
        .map(|s| s.architecture.as_str())
        .collect();
    assert_eq!(arches, ["x86_64", "arm64"]);
    assert_eq!(info.architectures[0].offset, 0x1000);
    assert_eq!(info.selected_architecture.as_deref(), Some("x86_64"));
    assert_eq!(info.architecture, "x86_64");
}

#[test]
fn fat_binary_arch_option_selects_a_slice() {
    let b = fat_fixture(&[
        (CPU_X86_64, macho64_fixture(CPU_X86_64, 2)),
        (CPU_ARM64, macho64_fixture(CPU_ARM64, 6)),
    ]);
    let opts = Options {
        arch: Some("ARM64".to_string()), // case-insensitive on purpose
        ..Options::default()
    };
    let info = parse(&b, &opts).expect("parses");
    assert_eq!(info.selected_architecture.as_deref(), Some("arm64"));
    assert_eq!(info.architecture, "arm64");
    assert_eq!(info.file_type, "Dynamic library (dylib)");
    assert_eq!(
        info.architectures.len(),
        2,
        "the full slice index stays visible whichever slice is reported"
    );
}

#[test]
fn fat_slice_symbol_and_string_tables_are_read_at_the_slice_offset() {
    // Mach-O file offsets inside a fat archive are relative to the SLICE, not
    // the archive; reading them as archive offsets yields garbage names.
    let b = fat_fixture(&[
        (CPU_X86_64, macho64_fixture(CPU_X86_64, 2)),
        (CPU_ARM64, macho64_fixture(CPU_ARM64, 2)),
    ]);
    let opts = Options {
        arch: Some("arm64".to_string()),
        ..Options::default()
    };
    let info = parse(&b, &opts).expect("parses");
    assert_eq!(info.imports.items, vec!["_printf".to_string()]);
    assert_eq!(info.exports.items, vec!["_main".to_string()]);
    assert_eq!(info.symbols.items[0].name, "_main");
    assert_eq!(info.libraries.items[0].name, "/usr/lib/libSystem.B.dylib");
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[test]
fn limit_caps_each_list_and_reports_the_pre_cap_total() {
    let opts = Options {
        limit: 2,
        ..Options::default()
    };
    let info = parse(&elf64_fixture(), &opts).expect("parses");
    assert_eq!(info.sections.items.len(), 2);
    assert_eq!(info.sections.total, 8);
    assert!(info.sections.truncated);
    assert_eq!(info.symbols.items.len(), 2);
    assert_eq!(info.symbols.total, 7);
    assert!(info.symbols.truncated);
    // Two libraries and a limit of two: full list, not flagged as truncated.
    assert_eq!(info.libraries.items.len(), 2);
    assert!(!info.libraries.truncated);
}

#[test]
fn limit_of_zero_clamps_up_rather_than_emptying_every_list() {
    let opts = Options {
        limit: 0,
        ..Options::default()
    };
    let info = parse(&elf64_fixture(), &opts).expect("parses");
    assert_eq!(info.sections.items.len(), 1);
    assert_eq!(info.sections.total, 8);
}

#[test]
fn limit_above_the_ceiling_clamps_to_max_limit() {
    let opts = Options {
        limit: usize::MAX,
        ..Options::default()
    };
    // Everything still fits, so the clamp is only observable as "no panic and
    // no truncation" — the ceiling itself is asserted by MAX_LIMIT.
    let info = parse(&elf64_fixture(), &opts).expect("parses");
    assert!(!info.sections.truncated);
    assert_eq!(MAX_LIMIT, 5000);
}

#[test]
fn switching_off_a_list_empties_it_without_touching_the_others() {
    let opts = Options {
        sections: false,
        symbols: true,
        imports: false,
        ..Options::default()
    };
    let info = parse(&elf64_fixture(), &opts).expect("parses");
    assert!(info.sections.items.is_empty(), "sections off");
    assert_eq!(info.sections.total, 0);
    assert!(!info.sections.truncated);
    assert!(info.libraries.items.is_empty(), "imports off covers libraries");
    assert!(info.imports.items.is_empty());
    assert!(info.exports.items.is_empty());
    assert_eq!(info.symbols.total, 7, "symbols stay on");
    // Header-level facts never depend on the list toggles.
    assert_eq!(info.architecture, "x86-64");
    assert_eq!(info.install_name.as_deref(), Some("libtest.so"));
}

// ---------------------------------------------------------------------------
// Error paths
// ---------------------------------------------------------------------------

#[test]
fn empty_input_is_rejected() {
    let e = parse(&[], &Options::default()).unwrap_err();
    assert!(e.contains("empty"), "{e}");
}

#[test]
fn input_shorter_than_a_magic_number_is_rejected() {
    let e = parse(b"\x7fEL", &Options::default()).unwrap_err();
    assert!(e.contains("too short"), "{e}");
}

#[test]
fn unrecognized_magic_names_the_bytes_it_found() {
    let e = parse(b"hello world, not a binary", &Options::default()).unwrap_err();
    assert!(e.contains("unrecognized binary"), "{e}");
    assert!(e.contains("68 65 6c 6c"), "the leading bytes are quoted: {e}");
}

#[test]
fn other_containers_get_a_pointed_hint_instead_of_a_bare_rejection() {
    for (bytes, hint) in [
        (&b"MZ\x90\x00"[..], "pe-info"),
        (&b"PK\x03\x04"[..], "ZIP archive"),
        (&b"!<arch>\n"[..], "static library"),
        (&b"\0asm\x01\0\0\0"[..], "WebAssembly"),
    ] {
        let e = parse(bytes, &Options::default()).unwrap_err();
        assert!(e.contains(hint), "expected {hint:?} hint, got: {e}");
    }
}

#[test]
fn elf_with_a_bad_class_byte_is_rejected() {
    let mut b = elf64_fixture();
    b[4] = 7;
    let e = parse(&b, &Options::default()).unwrap_err();
    assert!(e.contains("invalid ELF class byte 7"), "{e}");
}

#[test]
fn elf_with_a_bad_endianness_byte_is_rejected() {
    let mut b = elf64_fixture();
    b[5] = 9;
    let e = parse(&b, &Options::default()).unwrap_err();
    assert!(e.contains("invalid ELF data-encoding byte 9"), "{e}");
}

#[test]
fn truncated_elf_headers_are_rejected_with_the_sizes_involved() {
    let e = parse(b"\x7fELF\x02\x01\x01", &Options::default()).unwrap_err();
    assert!(e.contains("16-byte e_ident"), "{e}");

    let b = elf64_fixture();
    let e = parse(&b[..40], &Options::default()).unwrap_err();
    assert!(e.contains("40 bytes"), "{e}");
    assert!(e.contains("64-byte ELF64 header"), "{e}");
}

#[test]
fn a_java_class_file_is_not_mistaken_for_a_universal_binary() {
    // `cafebabe` is both the Mach-O fat magic and the Java class magic; a class
    // file's version numbers land where nfat_arch lives.
    let mut b = vec![0u8; 64];
    put_u32be(&mut b, 0, 0xcafe_babe);
    put_u16be(&mut b, 4, 0); // minor version
    put_u16be(&mut b, 6, 65); // major version 65 -> Java 21
    let e = parse(&b, &Options::default()).unwrap_err();
    assert!(e.contains("Java .class"), "{e}");
    assert!(e.contains("nfat_arch = 65"), "{e}");
}

#[test]
fn a_universal_binary_with_zero_slices_is_rejected() {
    let mut b = vec![0u8; 64];
    put_u32be(&mut b, 0, 0xcafe_babe);
    put_u32be(&mut b, 4, 0);
    let e = parse(&b, &Options::default()).unwrap_err();
    assert!(e.contains("nfat_arch = 0"), "{e}");
}

#[test]
fn asking_for_an_absent_architecture_lists_the_ones_present() {
    let b = fat_fixture(&[
        (CPU_X86_64, macho64_fixture(CPU_X86_64, 2)),
        (CPU_ARM64, macho64_fixture(CPU_ARM64, 2)),
    ]);
    let opts = Options {
        arch: Some("ppc64".to_string()),
        ..Options::default()
    };
    let e = parse(&b, &opts).unwrap_err();
    assert!(e.contains("\"ppc64\" is not in this universal binary"), "{e}");
    assert!(e.contains("x86_64, arm64"), "{e}");
}

#[test]
fn a_universal_slice_pointing_past_the_file_is_rejected() {
    let mut b = fat_fixture(&[(CPU_ARM64, macho64_fixture(CPU_ARM64, 2))]);
    put_u32be(&mut b, 8 + 8, 0x00ff_0000); // slice offset well past EOF
    let e = parse(&b, &Options::default()).unwrap_err();
    assert!(e.contains("starts at offset 16711680"), "{e}");
}

#[test]
fn a_universal_slice_that_is_not_mach_o_is_rejected() {
    let mut b = fat_fixture(&[(CPU_ARM64, macho64_fixture(CPU_ARM64, 2))]);
    put_u32(&mut b, 0x1000, 0xdead_beef); // clobber the slice's magic
    let e = parse(&b, &Options::default()).unwrap_err();
    assert!(e.contains("not a Mach-O slice"), "{e}");
    assert!(e.contains("0xdeadbeef"), "{e}");
}

#[test]
fn a_truncated_mach_o_header_is_rejected() {
    let b = macho64_fixture(CPU_ARM64, 2);
    let e = parse(&b[..20], &Options::default()).unwrap_err();
    assert!(e.contains("truncated Mach-O"), "{e}");
}

// ---------------------------------------------------------------------------
// Hostile input: nothing here may panic
// ---------------------------------------------------------------------------

#[test]
fn a_lying_elf_header_yields_a_result_or_an_error_but_never_a_panic() {
    let base = elf64_fixture();
    // Point every table offset/count at nonsense in turn.
    let pokes: [(usize, u64); 6] = [
        (32, u64::MAX),     // e_phoff
        (40, u64::MAX),     // e_shoff
        (40, 0x6fff_ffff),  // e_shoff just past EOF
        (24, u64::MAX),     // e_entry
        (E64_DYNAMIC as usize, u64::MAX), // a DT_ tag
        (E64_DYNSTR as usize, u64::MAX),  // clobbered string table
    ];
    for (off, val) in pokes {
        let mut b = base.clone();
        put_u64(&mut b, off, val);
        let _ = parse(&b, &Options::default());
    }
    // Absurd counts.
    let mut b = base.clone();
    put_u16(&mut b, 56, 0xffff); // e_phnum
    put_u16(&mut b, 60, 0xffff); // e_shnum
    let _ = parse(&b, &Options::default());

    // Every truncation of a valid file.
    for n in 0..base.len() {
        let _ = parse(&base[..n], &Options::default());
    }
}

#[test]
fn a_lying_mach_o_header_yields_a_result_or_an_error_but_never_a_panic() {
    let base = macho64_fixture(CPU_ARM64, 2);
    // ncmds far beyond what the file holds, and a zero cmdsize that would
    // otherwise spin the load-command walk forever.
    let mut b = base.clone();
    put_u32(&mut b, 16, 0xffff);
    let _ = parse(&b, &Options::default());

    let mut b = base.clone();
    put_u32(&mut b, 32 + 4, 0); // cmdsize = 0
    let info = parse(&b, &Options::default()).expect("a zero cmdsize stops the walk");
    assert!(info.sections.items.is_empty());

    let mut b = base.clone();
    put_u32(&mut b, 32 + 4, 0xffff_fff0); // cmdsize past EOF
    let _ = parse(&b, &Options::default());

    for n in 0..base.len() {
        let _ = parse(&base[..n], &Options::default());
    }
}

#[test]
fn a_string_table_with_no_terminator_is_bounded() {
    // A name offset into a table of non-NUL bytes must not run away.
    let mut b = elf64_fixture();
    for i in E64_DYNSTR..E64_DYNAMIC {
        b[i] = b'A';
    }
    let info = parse(&b, &Options::default()).expect("parses");
    for l in &info.libraries.items {
        assert!(l.name.len() <= 4096, "runaway string: {} bytes", l.name.len());
    }
}

// ---------------------------------------------------------------------------
// A genuinely real binary
// ---------------------------------------------------------------------------

/// The test executable itself is a real, toolchain-produced ELF; parsing it
/// proves the synthetic fixtures aren't a private dialect.
#[cfg(all(target_os = "linux", not(target_arch = "wasm32")))]
#[test]
fn parses_the_real_test_binary() {
    let Ok(bytes) = std::fs::read("/proc/self/exe") else {
        return; // no procfs — nothing to assert against
    };
    let opts = Options {
        limit: MAX_LIMIT,
        ..Options::default()
    };
    let info = parse(&bytes, &opts).expect("the test binary is a valid ELF");

    assert_eq!(info.format, "ELF");
    assert_eq!(info.bits, 64);
    assert_eq!(info.endianness, "little-endian");
    assert!(
        matches!(info.file_type.as_str(), "Executable" | "Shared object / PIE"),
        "unexpected file type {}",
        info.file_type
    );
    assert!(info.entry_point.unwrap_or(0) > 0, "a real binary has an entry point");
    assert!(
        info.sections.total > 5,
        "a real binary has a section table: {}",
        info.sections.total
    );
    assert!(
        info.sections.items.iter().any(|s| s.name == ".text"),
        "every real ELF has a .text"
    );
    assert!(
        info.sections
            .items
            .iter()
            .any(|s| s.name == ".text" && s.flags.contains(&"execinstr".to_string())),
        ".text must be flagged executable"
    );
    // A Rust test binary links libc dynamically on a normal gnu target; on a
    // fully static musl build it does not, so only assert consistency.
    if info.is_dynamic && !info.libraries.items.is_empty() {
        assert!(
            info.libraries.items.iter().any(|l| l.name.contains("lib")),
            "linked libraries look like sonames: {:?}",
            info.libraries.items
        );
        assert!(info.linker.is_some(), "a dynamic executable names its interpreter");
    }
    assert!(
        info.symbols.total > 0,
        "cargo test binaries keep a symbol table"
    );
}

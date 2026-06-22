# elf-info — competitor analysis (2026-06-23)

Snapshot for the new `elf-info` tool: parse an ELF (Executable and Linkable
Format) binary — a Linux/Unix executable, shared object (`.so`), relocatable
object (`.o`), or core dump — and report its identification + file header, the
section header table, the program header (segment) table, and the symbol table
(`.symtab` / `.dynsym`) as flat JSON.

## Surfaces verified (Phase 1)

- **Chat / LLM API:** `cargo test --workspace` green (5 core + 2 block: a
  drift-guard schema test + a build_resp mapping test). The drift-guard pins the
  authored chat schema (`Input::File` → `url`⊕`ref` `oneOf`) to `descriptor()`;
  `wafer build` validates the chat `block.wasm` (508.5 KiB, instantiates — pure
  Rust, no extra crates, so it runs on every backend incl. the chat Service
  Worker).
- **CLI:** `gizza tool elf-info url="https://busybox.net/downloads/binaries/1.21.1/busybox-x86_64"`
  → correct JSON: ELF64 / little-endian / System V / Executable / x86-64, entry
  `0x400b38`, 12 sections (`.init`/`.text`/`.rodata`/`.bss`/`.shstrtab` with
  decoded type + flags + address/offset/size), 3 segments (two `LOAD` with
  read/execute + read/write perms, `GNU_STACK`). The binary is static + stripped,
  correctly reflected as `is_dynamic=false`, no interpreter, 0 symbols.
- **Page:** none. A file→JSON report fits neither the pure-text page nor the
  ffmpeg file→media page shape — this is the no-page file-input pattern shared
  with `pe-info` / `detect-file-type` / `pdf-extract-text` (chat + CLI only).

## Competitors surveyed (paraphrased — no copy reused)

1. **`readelf` (GNU binutils)** — the reference ELF dumper. `-h` header, `-S`
   sections, `-l` program headers/segments, `-s` symbols, `-d` dynamic, `-r`
   relocations, `-n` notes, `--dyn-syms`. Exhaustive, but a local CLI with terse
   flag-driven output, not a paste-a-URL tool.
2. **`objdump` (GNU binutils)** — overlapping header/section/symbol dump plus
   disassembly (`-d`). Strength is the disassembler; the metadata view duplicates
   readelf.
3. **`llvm-readelf` / `llvm-objdump`** — LLVM's drop-in equivalents with
   readelf-compatible output; same local-CLI model.
4. **LIEF (Quarkslab)** — Python/C++ library to parse *and modify* ELF/PE/Mach-O;
   rich object model (segments, sections, dynamic entries, symbols, relocations,
   notes), used for instrumentation. A library, not a paste-and-go tool.
5. **pyelftools (eliben)** — pure-Python ELF/DWARF parser; the engine behind many
   web ELF viewers. Library; full DWARF debug-info support.
6. **Online ELF viewers (e.g. "ELF parser" web apps / `elfshaker`-style viewers)**
   — upload a binary, get a header/section/segment/symbol tree in the browser.
   Closest in shape to this tool; quality varies and many are JS re-implementations
   of pyelftools.

## Gap analysis (fit-to-model)

gizza tools are browser-local wasm, no account, no server. Against that filter:

**In-model — covered at launch (no gap to close):**
- Full `e_ident` + file header: ELF32/ELF64 class, **little- AND big-endian**
  byte order (both readers implemented and unit-tested with a big-endian PowerPC
  header), ELF version, OS/ABI (System V / Linux / FreeBSD / OpenBSD / Solaris /
  …), ABI version, file type (Relocatable / Executable / Shared object / Core),
  and machine architecture decoded by name (x86-64, AArch64/ARM64, RISC-V, MIPS,
  PowerPC, SPARC, LoongArch, BPF, AVR, …, ~30 IDs) with an `unknown (0xNN)`
  fallback so nothing is hidden.
- Section header table: name (resolved via `.shstrtab`), type (PROGBITS / SYMTAB
  / STRTAB / NOBITS / DYNAMIC / GNU_HASH / …), decoded flags (write / alloc /
  execinstr / merge / strings / tls / compressed), address, file offset, size.
- Program header (segment) table: type (LOAD / DYNAMIC / INTERP / NOTE / PHDR /
  TLS / GNU_STACK / GNU_RELRO / GNU_EH_FRAME / GNU_PROPERTY) with **read/write/
  execute permission flags**, offset, virtual address, file & memory sizes.
- Symbol table from **both `.symtab` and `.dynsym`**, each symbol with name
  (resolved via the section's linked string table), type (FUNC / OBJECT /
  SECTION / FILE / TLS / GNU_IFUNC), binding (LOCAL / GLOBAL / WEAK /
  GNU_UNIQUE), value (address) and size — capped at 8192 with a
  `symbols_truncated` flag so huge binaries can't blow up the JSON.
- Security-relevant derived facts surfaced directly: `is_pie_or_shared` (ET_DYN),
  `is_dynamic` (PT_DYNAMIC present), and the requested **program interpreter /
  dynamic linker** from `PT_INTERP` (e.g. `/lib64/ld-linux-x86-64.so.2`) — the
  same things a reverse-engineer reads first.
- A few **architecture-specific `e_flags`** decoded where they matter: ARM EABI
  version + hard/soft-float, RISC-V RVC + float ABI, MIPS PIC/CPIC.
- Bounds-checked, allocation-free header reads throughout (every multi-byte read
  is an `Option`), so a truncated or malformed file yields a graceful error or a
  partial-but-safe report rather than a panic — matched by `rejects_non_elf` and
  the truncation-tolerant section/segment loops.

**Out-of-model (considered, deliberately not built):**
- **Disassembly** (objdump `-d`) — a whole separate engine; out of scope for a
  metadata/triage tool.
- **DWARF debug-info** decode (pyelftools/LIEF) — a large independent format
  layered on top of ELF; the section table already names `.debug_*` sections.
- **Relocation tables** (`-r`) and full **dynamic section** entries (DT_NEEDED
  shared-library list, RPATH/RUNPATH, soname) — a reasonable future enhancement;
  this round reports the dynamic-linking *facts* (is_dynamic, interpreter) and
  the `.dynamic`/`.rela*` section entries, but not each relocation/`DT_` tag.
  Noted, not built, to keep the first cut focused on header+sections+segments+
  symbols (the tool's stated scope).
- **Binary editing / rewriting** (LIEF) — out of model; gizza tools are read-only
  analyzers.

## Conclusion

The launch implementation covers every in-model capability the readelf-class
dumpers and online ELF viewers expose for the header + sections + segments +
symbol-table scope this tool targets — across both ELF32/ELF64 and both byte
orders — plus the paste-a-URL, no-install, runs-in-the-browser delivery the
local CLIs and libraries lack. Relocation/dynamic-tag and DWARF decode are noted
as out-of-scope follow-ups, not gaps in the stated feature set. No additional
in-model gap to close this round.

## Sources

- [readelf (GNU binutils)](https://sourceware.org/binutils/docs/binutils/readelf.html)
- [objdump (GNU binutils)](https://sourceware.org/binutils/docs/binutils/objdump.html)
- [LLVM binary utilities](https://llvm.org/docs/CommandGuide/llvm-readelf.html)
- [LIEF](https://lief.re/)
- [pyelftools](https://github.com/eliben/pyelftools)
- [System V ABI / ELF specification](https://refspecs.linuxfoundation.org/elf/elf.pdf)

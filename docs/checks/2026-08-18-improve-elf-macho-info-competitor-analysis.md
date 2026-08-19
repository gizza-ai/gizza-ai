# Competitor analysis: elf-macho-info

Date: 2026-08-18
Tool: `elf-macho-info`
Backlog description: Parses ELF and Mach-O binaries to show sections, symbols, linked libraries, and architecture.

## Research method

Searched for: `online ELF Mach-O binary analyzer sections symbols linked libraries architecture tool`.
The extract backend was unavailable in this Hermes session, so this snapshot uses the search-result descriptions plus the current gizza model constraints. No competitor copy, branding, assets, or trademarks were copied.

## Competitors reviewed

| Competitor/tool page | What it appears to cover | Table-stakes found | UX patterns |
| --- | --- | --- | --- |
| Ewry binary analyzer | Browser-oriented binary analyzer for ELF, PE, and Mach-O | format identification, architecture, segments/sections, header metadata, symbols, file structure | file upload/drop, structured report |
| Edge Tools binary inspector | Browser-local ELF/PE/Mach-O inspector using a WASM binary parser | headers, sections, imported libraries/functions, full JSON, no-upload positioning | file upload/drop, structured tabs/JSON export |
| JavaInUse ELF analyzer | ELF/SO-focused web inspector | ELF header, sections, symbols, dependencies/DT_NEEDED, strings, common CPU architectures | upload, separate report sections |
| Fatcousin ELF binary analyzer | Browser-local ELF analyzer | architecture and sections with no-upload/privacy positioning | drag/drop upload, concise structured output |
| AnyOnlineTool binary file analyzer | Generic binary/file structure analyzer | file signatures, endianness, basic executable-format metadata across PE/ELF/Mach-O/archive formats | upload, broad static-analysis framing |

## Fit-to-model decisions

### In model and implemented

- ELF and Mach-O magic sniffing with clear errors for empty/truncated/unrecognized inputs.
- Common ELF metadata: class/word size, endianness, OS ABI, machine/architecture, file type, entry point, PIE/dynamic/stripped signals, interpreter, SONAME, RPATH/RUNPATH, section table, symbol tables, dynamic imports/exports, linked libraries.
- Common Mach-O metadata: 32/64-bit, endianness, CPU/subtype architecture, file type, entry point, PIE/dynamic/stripped signals, dynamic linker, install name, UUID, deployment platform, load-dylib entries with versions, rpaths, sections, symbols/imports/exports.
- Fat/universal Mach-O support: list slices and optionally select a slice by architecture.
- Report limiting controls: `sections`, `symbols`, `imports`, `limit`, and `arch` so a huge symbol table can be summarized safely.
- Deterministic plain-text report plus structured JSON fields for LLM/CLI consumers.
- Browser-local/wasm-safe parsing: no server-side analysis, no execution of the uploaded binary.
- Helpful hints for adjacent formats such as PE/DOS executables, ZIP archives, ar archives, and WebAssembly modules.

### Considered, not built

- PE/COFF parsing: already covered by separate PE-oriented tooling in this repo family and would make this tool less focused.
- Recursive directory scans: out of model for a single gizza block invocation.
- Deep security-hardening checks such as RELRO/canary/NX/code-signing/notarization scoring: useful but broader than this backlog item; can be a later dedicated improvement.
- Strings extraction and disassembly: table-stakes in some desktop reverse-engineering tools, but large/noisy outputs and instruction decoding exceed the focused metadata parser.
- Server-side malware scanning, reputation, signatures, unpacking, and debug-symbol lookup: require network services/accounts or executing heavyweight backends, so they do not fit browser-local wasm.
- Standalone page upload UI: current gizza page patterns cover pure parameter-only tools and ffmpeg media transforms; arbitrary file-to-structured-report tools in this repo ship as chat/CLI no-page blocks.

## Resulting descriptor/CLI expectations

- Required file source: `url` or `ref` from the shared `Input::File` schema.
- Optional booleans: `sections`, `symbols`, `imports`, all default true.
- Optional integer: `limit`, clamped to 1-5000 with default 100.
- Optional string: `arch` for fat/universal Mach-O slice selection.
- Output: `report` plus flat JSON fields for format, bits, endianness, architecture, file type, flags, libraries, sections, symbols, imports, exports, slices, and byte count.

## Verification focus from the scan

- Unit tests must cover ELF headers, dynamic library metadata, sections, symbols, imports/exports, Mach-O headers/load commands, fat Mach-O slice selection, truncation/errors, and cap behavior.
- CLI verification should assert exact recognizable report content from a real ELF/Mach-O input.
- Hygiene should confirm descriptor parameters stay synced into `manifest.json` even though there is no standalone page.

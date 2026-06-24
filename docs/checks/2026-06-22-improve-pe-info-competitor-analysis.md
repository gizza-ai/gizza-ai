# pe-info — competitor analysis (2026-06-22)

Tool: `pe-info` — parse a Windows Portable Executable (`.exe` / `.dll` / `.sys`)
and report headers, sections, imports, and exports. Pure-Rust, dependency-free
parser. Surfaces: chat + CLI (file→JSON, no standalone page — the
detect-file-type / web-fetch pattern).

## Scope of this snapshot

This is a *new* backlog tool, so the analysis is forward-looking: it surveys the
established PE-inspection tools to make sure the first cut covers the features
users actually reach for, and records the (mostly out-of-model) gaps. All
findings are paraphrased; no competitor copy, branding, or fixtures were used.

## Competitors surveyed

1. **pefile (erocarrera/pefile)** — the de-facto Python library + `pefile`
   examples. Parses DOS/NT headers, optional header, data directories, the full
   section table, import/export/resource/relocation/debug/TLS directories,
   and computes section entropy + hashes. Deep, exhaustive, programmatic.
2. **CFF Explorer / PE-bear / PEview (GUI inspectors)** — interactive
   header/section/directory tree views with hex panes, import/export tables,
   resource viewers, and hex-edit. Strong UX, Windows-desktop only.
3. **`dumpbin` (MSVC) / `objdump -p` (binutils)** — CLI header + import/export
   dumps; dumpbin also disassembles and lists dependents (`/IMPORTS`,
   `/EXPORTS`, `/HEADERS`).
4. **VirusTotal / file-analysis web UIs** — upload a binary, get PE metadata
   (machine, subsystem, timestamp, sections w/ entropy, imports/exports,
   signatures) alongside AV verdicts. Server-side, account-gated for batch.
5. **`pedump` / `pelook` style web "PE header viewer" pages** — paste/upload an
   EXE and render the header fields, section table, and import list in a table.

## Feature diff vs. our first cut

| Capability | Competitors | pe-info (ours) | Verdict |
|---|---|---|---|
| PE32 vs PE32+ detect | yes | **yes** | covered |
| Machine / arch | yes | **yes** (x86/x64/ARM/ARM64/IA-64/RISC-V/EFI) | covered |
| Kind (exe/dll/driver) | yes | **yes** (Executable/DLL/Driver via characteristics) | covered |
| Subsystem | yes | **yes** (GUI/Console/Native/EFI/…) | covered |
| Link timestamp | raw + sometimes decoded | **yes** (raw + ISO-8601 UTC, no chrono dep) | covered |
| Image base / entry point | yes | **yes** (hex) | covered |
| SizeOfImage | yes | **yes** | covered |
| DLL characteristics (ASLR/DEP/CFG) | pefile/dumpbin | **yes** (decoded flag list) | covered |
| Section table (name/VA/sizes/flags) | yes | **yes** (decoded access flags) | covered |
| Import table (lib + symbols, ordinals) | yes | **yes** (named + `#N (ordinal)`, ILT-preferred) | covered |
| Export table (names + internal DLL name) | yes | **yes** | covered |
| Resource directory tree | pefile/CFF | no | out-of-model-ish (large; low LLM value) — deferred |
| Section entropy / hashes (imphash) | pefile/VT | no | **candidate follow-up** (pure-Rust feasible) |
| Authenticode signature parse | VT/CFF | no | deferred (heavy ASN.1/PKCS#7; low value here) |
| Relocations / TLS / debug dirs | pefile | no | deferred (niche; verbose) |
| Disassembly | dumpbin/objdump | no | out of scope (needs a disassembler engine) |
| AV verdict | VirusTotal | no | out of model (server + threat-intel backend) |
| Batch / account / API | VT | no | out of model (browser-local, no account) |

## In-model gaps closed in this build

The first cut already ships the union of what header-viewer competitors expose:
format, machine, kind, subsystem, timestamps (raw **and** decoded), image
base/entry point, image size, decoded DLL-characteristics mitigations, the full
section table with decoded access flags, the import table (named symbols **and**
ordinal imports, reading the Import Lookup Table when present and falling back to
the IAT), and the export table (symbol names + the internal export DLL name).
Output is capped (≤256 import libs, ≤4096 funcs/lib, ≤8192 exports) so a giant
binary can't produce megabytes of JSON.

## In-model follow-ups considered, not built (kept honest)

- **Section entropy + imphash / SHA-256** — pure-Rust feasible and useful for
  malware triage; deferred to keep the first cut focused. Good next iteration.
- **Resource directory summary (version info / icons present)** — feasible but
  verbose; low signal for the LLM. Deferred.

## Out-of-model (recorded, not built)

- **Authenticode / signature verification** — needs PKCS#7/ASN.1 + a trust
  store; heavy and low value in a browser-local tool.
- **Disassembly** — needs a disassembler engine; out of scope for a metadata
  tool.
- **AV verdicts / threat intel / batch / accounts (VirusTotal-style)** — require
  a server backend; out of gizza's no-account, no-server, browser-local model.

## Verification

- Unit tests (core + block): 6 passing — synthetic PE32+ with an import table,
  non-PE rejection, machine/subsystem tables, ISO-8601 timestamp formatting,
  drift-guard schema (no LLM-facing chat-schema drift), and the core→Resp mapping.
- `wafer build`: chat block instantiates clean (no FS/WASI imports).
- CLI live tests:
  - PE32 NSIS installer (nmap-7.94-setup.exe) → PE32 / x86 / Windows GUI / 5
    sections / 7 import libs with named + ordinal symbols / ASLR+DEP flags.
  - PE32+ DLL (mimalloc-redirect.dll) → PE32+ / x86-64 / DLL / image base
    0x180000000 / 5 exports / internal export DLL name / 1 import lib.
- No standalone page (file→JSON shape); chat + CLI are the supported surfaces.

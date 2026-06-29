# lznt1-decompress — competitor analysis & surface checks (2026-06-29)

**Tool:** `lznt1-decompress` — decompress Microsoft LZNT1 blobs produced by Windows `RtlCompressBuffer` / `RtlDecompressBuffer` (`COMPRESSION_FORMAT_LZNT1`). Pure Rust decoder for chunk headers, flag groups, literals, and back-references; runs on chat block, CLI, and browser page.

## Surface verification (all green)

| Surface | Check | Result |
| --- | --- | --- |
| Core + descriptor tests | `cd blocks/lznt1-decompress && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 12 core tests + 1 drift-guard schema test pass |
| Chat block (wasm32-wasip1) | `cd blocks/lznt1-decompress && CARGO_BUILD_JOBS=1 wafer build` | ✅ OK, `target/block.wasm` validates/instantiates (307.9 KiB) |
| Page wasm (wasm32-unknown-unknown) | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/lznt1-decompress/web --target web --release --out-dir pkg` | ✅ pkg built |
| CLI | `gizza tool lznt1-decompress data=05b0084142430020 input_encoding=hex output_encoding=text` | ✅ returns `ABCABC`; stored chunk returns `414243` as hex |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/lznt1-decompress/` |
| Page (Playwright) | `tool-page-lznt1-decompress.spec.ts` | ✅ 2 passed |

The chat schema is single-sourced from `descriptor()` and locked by the `schema_json_matches_authored_chat_schema` drift test.

## Competitor landscape

Top references/tools users reach for:

1. **Microsoft `RtlDecompressBuffer` documentation** — canonical API reference for LZNT1 decompression on Windows.
2. **`rustyx/unpack_lznt1`** — small Windows command-line tool that calls `RtlDecompressBuffer`.
3. **CyberChef feature discussions / forensic workflows** — LZNT1 appears in malware config and NTFS/Windows artifacts; demand is often analyst-driven.
4. **AutoIt / PowerShell snippets calling ntdll** — platform-specific decompression helpers around the Windows API.
5. **ServerFault / reverse-engineering answers** — ad-hoc tooling guidance for binary blobs compressed with Microsoft LZNT1.

## Capability diff

| Capability | Competitors | gizza lznt1-decompress |
| --- | --- | --- |
| Decompress LZNT1 chunk stream | Windows API / tools | ✅ pure Rust implementation |
| Works off Windows | many API wrappers require Windows | ✅ wasm/CLI, no ntdll dependency |
| Hex input | analyst scripts/tools | ✅ default, whitespace + `0x` prefix tolerated |
| Base64 input | some wrappers | ✅ |
| Binary-safe output | tools vary | ✅ hex (default) or Base64 |
| Text output for UTF-8 blobs | tools vary | ✅ explicit `output_encoding=text` |
| Error handling for malformed streams | varies | ✅ truncation, bad backrefs, bad encodings tested |
| Compression / recompression | Windows API can compress | ❌ out of model for this decompressor |
| Fragment/windowed decompression | `RtlDecompressFragment` exists | ❌ out of scope |
| File offset skip/carving | some CLI tools | ❌ use surrounding extraction tools, then paste blob |

## In-model gaps closed / confirmed

- Implemented full chunk-stream decompression including compressed and stored chunks.
- Added the dynamic LZNT1 token split based on current chunk output length, including overlapping back-reference copies.
- Added robust input encoding handling for analyst-friendly hex and Base64.
- Added output rendering as hex, UTF-8 text, or Base64 to handle both binary artifacts and readable configs.
- Added tests for literals, back-references, overlapping copies, stored chunks, malformed input, and surface-level hex/Base64 conversions.
- Added Playwright coverage for direct page usage and query-param deep-linking.

## Out-of-model (intentionally not built)

- **LZNT1 compression** — useful but a separate tool; this backlog item is decompression-focused for analysis workflows.
- **RtlDecompressFragment-style partial extraction** — requires caller-provided fragment offsets/window semantics; out of scope for a paste-and-decode utility.
- **File carving / registry hive parsing** — surrounding forensic parsing belongs to dedicated artifact tools; this tool decodes a blob already identified as LZNT1.

No competitor copy, branding, or assets were used.

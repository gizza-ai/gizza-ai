# lz-string-compress — competitor analysis & surface checks (2026-06-29)

**Tool:** `lz-string-compress` — compress or decompress strings with the LZ-String algorithm, supporting Base64, URL-safe, and UTF-16 transport encodings. Pure Rust (`lz-str`), runs on chat block, CLI, and browser page.

## Surface verification (all green)

| Surface | Check | Result |
| --- | --- | --- |
| Core + descriptor tests | `cd blocks/lz-string-compress && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 15 core tests + 1 drift-guard schema test pass |
| Chat block (wasm32-wasip1) | `cd blocks/lz-string-compress && CARGO_BUILD_JOBS=1 wafer build` | ✅ OK, `target/block.wasm` validates/instantiates (334.1 KiB) |
| Page wasm (wasm32-unknown-unknown) | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/lz-string-compress/web --target web --release --out-dir pkg` | ✅ pkg built |
| CLI | compress/decompress `Hello` in Base64 and `Hello world` in URI format | ✅ byte-compatible vectors and round-trip verified |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/lz-string-compress/` |
| Page (Playwright) | `tool-page-lz-string-compress.spec.ts` | ✅ 4 passed |

The chat schema is single-sourced from `descriptor()` and locked by the `schema_json_matches_authored_chat_schema` drift test.

## Competitor landscape

Top references/tools users reach for:

1. **pieroxy LZ-String project/demo** — canonical JavaScript library and documentation, including Base64/URI/UTF-16 encoders.
2. **npm `lz-string` / CLI wrappers** — package used by browser and Node apps for compact localStorage/URL payloads.
3. **CodePen / small online LZString demos** — ad-hoc `compressToBase64` / `decompressFromBase64` forms for testing payloads.
4. **Rust `lz-str` crate** — Rust port used as the implementation substrate here.
5. **Generic Base64/URL encoders** — adjacent tools users often confuse with compression; useful for transport but not equivalent.

## Capability diff

| Capability | Competitors | gizza lz-string-compress |
| --- | --- | --- |
| LZ-String compress/decompress | canonical library/demos | ✅ |
| Base64 transport form | canonical library | ✅ normalized to JS-compatible padding |
| URL-safe encodedURIComponent form | canonical library | ✅ |
| UTF-16 localStorage form | canonical library | ✅ |
| Unicode text round-trip | canonical library | ✅ tests cover accents/CJK/emoji |
| Invalid-payload errors | varies; some decoders are lenient | ✅ rejects out-of-alphabet Base64/URI garbage before decoding |
| Browser-local/private conversion | demos vary | ✅ wasm page, no upload |
| CLI + chat/LLM API | uncommon | ✅ `gizza tool` and chat block |
| Raw `compress()` unpaired UTF-16 output | canonical library | ❌ intentionally omitted; transport-safe forms only |
| Streaming/large-file compression | CLI/library ecosystems | ❌ out of model |

## In-model gaps closed / confirmed

- Added mode selection (`compress`/`decompress`) and format selection (`base64`, `uri`, `utf16`).
- Normalized Base64 padding so output matches the canonical JS `LZString.compressToBase64` textual form.
- Added reference vectors (`Hello`, sample sentence, URI form) and Unicode round-trip tests.
- Hardened decompression against garbage input: the underlying crate silently drops out-of-alphabet Base64/URI characters, so the tool validates alphabets first and surfaces an honest error instead of returning an empty string.
- Added Playwright coverage for default Base64, URL-safe output, round-trip, and garbage decompression error handling.

## Out-of-model (intentionally not built)

- **Raw `compress()` output** — produces arbitrary UTF-16 code units that are unsafe for URLs and many text surfaces; this tool focuses on transport-safe encodings.
- **File/binary compression** — LZ-String is text-oriented; binary/large-file compression belongs to archive/codec tools.
- **Streaming compression** — unnecessary for this stateless string utility and not supported by the LZ-String API shape.

No competitor copy, branding, or assets were used.

# magnet-link-parser — competitor analysis & surface checks (2026-06-29)

**Tool:** `magnet-link-parser` — parse a BitTorrent `magnet:` URI into structured fields, or build a magnet link from parts.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/workspace tests | `cd blocks/magnet-link-parser && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 19 tests passed (descriptor drift guard + parser/builder/core cases) |
| Chat block | `cd blocks/magnet-link-parser && CARGO_BUILD_JOBS=1 wafer build` | ✅ produced and validated `target/block.wasm` |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/magnet-link-parser/web --target web --release --out-dir pkg` | ✅ built `web/pkg` |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/magnet-link-parser/` |
| CLI parse | `gizza tool magnet-link-parser magnet='magnet:?xt=urn:btih:...&dn=Some+File&tr=...&xl=1048576'` | ✅ returned parsed JSON with info hash, display name, tracker, exact length |
| CLI build | `gizza tool magnet-link-parser mode=build info_hash=... display_name='My File' trackers='udp://a.example:1337'` | ✅ returned encoded `magnet:?xt=urn:btih:...&dn=My%20File&tr=...` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-magnet-link-parser.spec.ts` | ✅ 3 passed (parse, build, query-param deep-link) |

## Competitor scan

Searches reviewed:
- `online magnet link parser decoder builder competitors magnet URI info hash tracker`
- `magnet link generator parser BitTorrent online tool`

Representative competitors and references:

1. **RapidToolSet Magnet Link Validator** — validates magnet links, parses components, and highlights info hash, display name, and tracker URLs; claims local processing.
2. **RapidToolSet Magnet Link Decoder** — standalone decoder for info hash, display name, file size, trackers, and related metadata; claims local processing.
3. **TorBox Tools** — torrent-file-to-magnet conversion tool that runs locally and copies/displays a magnet link.
4. **4ndv/magnets** — open-source online magnet links editor.
5. **AnyOnlineTool Magnet Link Converter** — parses magnet links and extracts info hashes, trackers, and metadata.

## Gap / fit analysis

| Capability | Competitors | gizza `magnet-link-parser` | Decision |
| --- | --- | --- | --- |
| Decode basic magnet fields | Most tools extract `xt`, `dn`, `tr` | ✅ parses `xt`, `dn`, repeated `tr`, and indexed keys like `tr.1` | Built |
| Info-hash normalization | Decoders show the info hash; some accept both hex/base32 | ✅ normalizes v1 `btih` from 40-hex or 32-base32 to lower-case hex and records the source encoding | Built |
| BitTorrent v2 awareness | Less common in small decoders | ✅ recognizes `urn:btmh:` as `info_hash_v2` | Built |
| Additional BEP/de-facto fields | Better editors preserve more fields | ✅ handles web seeds (`ws`), acceptable/exact sources (`as`/`xs`), keywords (`kt`), exact length (`xl`), and unknown params | Built |
| Build/edit magnet links | Editors/converters can assemble links | ✅ build mode creates encoded `magnet:?` URIs from info hash, display name, tracker list, web seeds, exact length | Built |
| Torrent-file upload to magnet | TorBox converts `.torrent` files | ❌ out-of-model for this text-only pure tool; requires bencode/torrent file parsing and upload UX | Not built |
| Network validation / tracker probing | Some BitTorrent clients can test trackers | ❌ out-of-model: network calls and tracker protocol access | Not built |
| Privacy/local execution | Several competitors emphasize local processing | ✅ pure wasm/page + CLI/chat; no network needed | Built |

## Improvements made from analysis

- Added both parse and build modes instead of parse-only output.
- Included human-readable page output for parse mode while keeping JSON on chat/CLI surfaces.
- Preserved lesser-known fields and unknown parameters so the tool works as a diagnostic/editor aid, not just a simple info-hash extractor.
- Added query-param page coverage for shareable parser links.

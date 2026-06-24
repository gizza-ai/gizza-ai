# extract-mac-addresses — competitor analysis (2026-06-23)

Tool: find MAC (EUI-48 / EUI-64) addresses in pasted text or logs written in any
common notation, normalize them all to one chosen output format, deduplicate by
underlying bytes, return first-seen order. Pure-Rust → chat + CLI + page.

## Surfaces verified

- **Chat block**: `wafer build` validates `target/block.wasm` (1505 KiB). Schema
  drift-guard unit test passes.
- **CLI**: `gizza tool extract-mac-addresses text=… [format=…]` → JSON
  `{count, macs}`. Verified colon (default), cisco format, and empty cases.
- **Page** `/tools/extract-mac-addresses/`: 3 Playwright tests pass (multi-notation
  dedup + normalization, cisco output format select, no-match message).

## Competitors surveyed

1. **The Text Tool** (thetexttool.com/tools/extract-mac-addresses) — extracts +
   normalizes to multiple formats, deduplicates, runs in-browser, private/free.
2. **IPVoid MAC Address Extractor** (ipvoid.com/mac-address-extractor) —
   paste-and-extract, server-side.
3. **APIVoid MAC Address Extractor** (apivoid.com/tools/extract-mac-addresses) —
   detects MAC patterns in logs/code/raw data; also an API.
4. **toolpage.org MAC Extractor** (en.toolpage.org/tool/mac-extractor) — basic
   extract from entered text.
5. **iHateRegex / RegexForge MAC patterns** — reference regexes (the IEEE 802
   forms: hyphen, colon, and Cisco three-group dotted).

## Capability diff vs. gizza

| Capability | Competitors | gizza extract-mac-addresses |
| --- | --- | --- |
| Colon notation `00:1a:..` | yes | yes |
| Hyphen notation `00-1a-..` | yes | yes |
| Cisco dotted-quad `001a.2b3c.4d5e` | most | yes |
| Bare hex `001a2b3c4d5e` | some | yes |
| EUI-64 (8-byte / 16-hex) | rare | yes (all four notations) |
| Normalize to a chosen output format | The Text Tool only | yes (colon/hyphen/cisco/bare) |
| Deduplicate by bytes (not by string) | The Text Tool | yes (same addr two notations → one) |
| Reject 32-char hash / long hex false positives | varies | yes (non-hex boundary guard) |
| Runs locally / private | The Text Tool only | yes (all surfaces, pure-Rust wasm) |
| Available in chat + CLI + page | none | yes |

## Gaps considered

- **In-model, addressed**: multi-notation input, normalization to a single
  format, byte-level dedup, EUI-64, and hash/long-hex false-positive rejection —
  all implemented and tested.
- **Out of scope / not built** (would need network or a bundled OUI database,
  which is a separate tool — `mac-vendor-lookup` already exists in this repo):
  - OUI / vendor (manufacturer) lookup per address.
  - Locally-administered / multicast bit flags, broadcast detection.
  These are deliberately left to the dedicated `mac-vendor-lookup` block rather
  than duplicated here; this tool stays a pure offline extractor/normalizer.

## Conclusion

Coverage meets or exceeds the surveyed extractors: it matches the most capable
competitor (The Text Tool: multi-format normalize + dedup + private) and adds
EUI-64 support, a hash false-positive guard, and three delivery surfaces
(chat/CLI/page). No in-model capability gaps remain. No competitor copy,
branding, or trademarks were used.

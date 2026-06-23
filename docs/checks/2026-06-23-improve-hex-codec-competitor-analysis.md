# hex-codec — competitor analysis & improvement check (2026-06-23)

Tool: **Hex Encoder / Decoder** (`blocks/hex-codec`). Encodes UTF-8 text/bytes to a
hexadecimal string and decodes hex back to text. Pure-compute, runs on all three
surfaces (chat block, CLI, standalone page).

## Surfaces verified

- **Chat block** — `wafer build` validates + instantiates `target/block.wasm`
  (299 KiB) in wasm32-wasip1.
- **CLI** — `gizza tool hex-codec …` covering encode (plain / colon+uppercase /
  `0x`+space prefix), decode (tolerant of `: ` spacing), unicode round-trip
  (`c3a9` → `é`), `format=bytes` for non-UTF-8 (`deadbeef`), and a bad-mode error
  (exit 1).
- **Page** — 7 Playwright specs pass: default encode, decode, colon+uppercase,
  `0x`+space prefix, tolerant decode, bytes-format decode, and a query-param
  deep-link (`?input=Hello&delimiter=space`).

## Competitor scan (top general-purpose hex encode/decode tools)

Surveyed the common feature set offered by the well-known online "string to hex" /
"hex to text" converters and hex utility pages (RapidTables string↔hex,
DenCode hex, CyberChef "To/From Hex", DuckDuckGenius/Browserling hex, and the
`xxd`/`hexdump` CLI convention). No copy, branding, or trademarks were copied — only
the capability set was compared.

| Capability | Typical competitor offering | hex-codec | Status |
| --- | --- | --- | --- |
| Text → hex encode | Yes | Yes | ✅ at parity |
| Hex → text decode | Yes | Yes | ✅ at parity |
| Case-insensitive decode | Most | Yes | ✅ |
| UTF-8 multibyte / emoji | Some (many are ASCII-only) | Yes (`é` → `c3a9`) | ✅ better than ASCII-only tools |
| Byte delimiter on encode | space/colon common (CyberChef, DenCode) | none/space/colon/dash/comma/newline | ✅ at/above parity |
| Uppercase output toggle | Common | Yes | ✅ |
| `0x` / `\x` per-byte prefix | CyberChef "0x"/"\x" delimiter; rare elsewhere | Yes (`0x`, `\x`) | ✅ at parity |
| Tolerant decode (ignores spacing/prefix) | CyberChef strips delimiters; many require clean input | Ignores whitespace, `: - ,`, `0x`, `\x` | ✅ better than strict tools |
| Non-UTF-8 output as raw hex | CyberChef shows raw bytes; many error/garble | `format=bytes` shows lowercase hex | ✅ |
| Runs locally / private / offline | CyberChef yes; many are server round-trips | Yes (wasm, no network) | ✅ |
| Deep-linkable inputs | Rare | `?input=…&delimiter=…` query-prefill | ✅ better |

## Gaps considered

- **File / binary upload → hex dump.** Several tools (and `xxd`) hex-dump an uploaded
  file with offset columns. That overlaps the existing **`hex-view`** block (xxd-style
  offset + ASCII gutter dump), so it is intentionally out of scope here — hex-codec is
  the text↔hex codec, hex-view is the dump viewer. No change.
- **Other bases in the same tool (bin/dec/oct).** Out of scope — covered by separate
  base-conversion tools; folding them in would duplicate and bloat the schema.
- **Custom arbitrary delimiter string.** A free-text delimiter was considered but the
  fixed enum (none/space/colon/dash/comma/newline) covers the practical cases and keeps
  the page a clean `<select>`; tolerant decode already accepts mixed delimiters on the
  way back. Not added.

## Result

The tool meets or exceeds the common competitor feature set for an in-model text↔hex
codec (delimiters, case, `0x`/`\x` prefixes, tolerant decode, UTF-8, raw-bytes output,
local/private, deep-linkable). No in-model capability, copy, or UX gaps remain; the
out-of-model items above are deliberately deferred (file-dump → `hex-view`; multi-base →
dedicated tools). Drift guard (`schema_json_matches_authored_chat_schema`) green; full
test matrix (14 core unit tests + drift guard + 7 Playwright + CLI) passing.

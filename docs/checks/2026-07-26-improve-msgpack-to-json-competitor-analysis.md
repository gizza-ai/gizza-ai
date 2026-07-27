# msgpack-to-json — competitor analysis (2026-07-26)

Tool: decode a MessagePack binary blob (given as hex or base64) into
pretty-printed JSON for inspection. Browser-local wasm, no upload. Type: pure.

## Competitors scanned (paraphrased; no copy/branding reproduced)

1. **MsgPack Converter (ref45638.github.io)** — decode MsgPack from **Base64,
   Hex, or Go `[]byte`** into JSON, and encode JSON back to MsgPack. Two-way,
   client-side.
2. **MsgPack Converter (sf-zhou.github.io)** — Base64 MsgPack ↔ JSON with
   explicit **full uint64 support** called out as a differentiator.
3. **jsonparser.ai / MessagePack to JSON** — paste MessagePack **hex or base64**,
   get decoded JSON instantly; **browser-only, bytes never leave the tab**.
4. **Flipper File MessagePack Decoder** — paste **hex, base64, or upload a
   `.msgpack` file**; no login, 100% in-browser.
5. **ToolsLick MessagePack to JSON** — converts MessagePack in **HEX, UInt8Array,
   Base64, or Percent Encoding** into formatted JSON; LDDGO offers the same set of
   input encodings plus JSON→msgpack encoding.

## Table-stakes params / defaults / UX (tagged in-model / out-of-model)

| Capability | Competitor | Our decision |
|---|---|---|
| Input as **hex** | all | in-model — `input_format=hex` (separators/`0x` tolerated) |
| Input as **base64** | all | in-model — `input_format=base64` (std + URL-safe, padding optional) |
| **Auto-detect** hex vs base64 | jsonparser.ai | in-model — `input_format=auto` (default) |
| **Pretty-print / indent** (spaces, minify) | all | in-model — `indent` (0-8, default 2; 0 = minified) |
| **Full uint64 precision** (no float rounding) | sf-zhou (headline) | in-model — u64 kept exactly via serde_json |
| Preserve **map/field order** | implicit | in-model — `preserve_order` feature |
| Handle msgpack-only types (**bin**, **ext**, **timestamp**) | implicit in decoders | in-model — bin/ext → base64/hex string (`binary_format`); ext → `{$ext,data}`; timestamp ext → RFC 3339 |
| Decode a **stream** of several values | msgpack-tools `msgpack2json` | in-model — concatenated values → JSON array |
| Actionable **errors** on bad input | most | in-model — byte-offset error for bad hex/base64 or malformed msgpack |
| Example presets / sample loader | several | in-model — `[[example]]` chips |
| `?input=` URL prefill | generic tool pages | in-model — native query-param deep-links |
| Copy / Download output | all | in-model (generic) — page `format=text` gets a Download link; copy is platform chrome |
| Input as **Go `[]byte`** / **UInt8Array** / **percent-encoding** | ref45638, ToolsLick | out-of-model — niche paste forms; hex/base64 cover the same bytes. Listed, not built |
| **File upload** (`.msgpack`) | Flipper File | out-of-model — pure page is a paste field; users hex/base64-encode the file. Listed, not built |
| **JSON → MessagePack** (encode direction) | ref45638, sf-zhou, LDDGO | out-of-model here — this tool is decode-only per backlog (a separate json-to-msgpack tool would cover it) |

## Design decisions

- `input_format=auto` is the default: the tool detects hex (even-length run of
  hex digits after stripping whitespace/`:`/`-`/`,`/`0x`) vs base64, so users can
  paste either without a mode switch; `hex`/`base64` force it when ambiguous.
- `indent` (0-8, default 2) covers the pretty/minify axis; 0 = one compact line,
  matching the "minify" option competitors ship.
- Full **uint64** precision and preserved **map order** are kept (both called out
  by competitors), via serde_json's arbitrary-precision integers +
  `preserve_order`.
- MessagePack types with no JSON equivalent use a documented convention rather
  than silently dropping data: `bin`/`ext` payloads render as a base64 (default)
  or hex string (`binary_format`), an unknown `ext` becomes
  `{"$ext": <type>, "data": "<encoded>"}`, and the reserved **timestamp**
  extension (type -1; 4/8/12-byte forms) becomes an RFC 3339 UTC string.
- Non-finite floats (NaN/Infinity) → `null`; non-string map keys are stringified;
  a concatenated stream of values → a JSON array — all documented on the page.

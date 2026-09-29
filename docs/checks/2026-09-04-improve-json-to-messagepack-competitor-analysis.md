# json-to-messagepack — competitor analysis (2026-09-04)

Scan run BEFORE implementation, so the descriptor could ship the table-stakes from day one.
All findings are paraphrased observations of publicly visible behaviour. No competitor copy,
markup, branding, or trademark is reproduced here or in the tool.

## Competitors reviewed (top 3 reachable, real tools)

1. **openformatter.com/msgpack-encode** — browser-local JSON → MessagePack encoder. Output can be
   shown as Base64 or Hex (toggle buttons above the output panel). Shows a live byte-count
   comparison of the JSON input vs the MessagePack output plus a percentage saving. Ships a
   loadable sample telemetry document, and an annotated worked example that names the type tags
   (fixmap / fixstr / positive fixint / true) byte by byte. Long FAQ covering what MessagePack is,
   when to prefer it over JSON, why the output is text-encoded, typical size reduction, type
   coverage, float encoding (float64), privacy, and how to decode again.
2. **jsonutils.org/json-to-msgpack.html** — JSON → MessagePack with a 16-byte-per-row hex dump, a
   Base64 view, independent "show hex dump" / "show Base64" toggles, a JSON vs MessagePack byte
   count with a percentage reduction, plus Load Sample / Clear buttons and a `.msgpack` binary
   download. Encoding happens in the page.
3. **ref45638.github.io/msgpack-converter** — bidirectional converter with an encoding selector
   offering Base64, hexadecimal and a byte-array (`[]byte`-style) representation, copy buttons on
   both panels, and a direction switch between decode and encode. Client-side only.

(A fourth result, msgpackconverter.com, did not resolve at scan time and was replaced by
ref45638's converter so three reachable tools were still reviewed.)

## Table stakes → where each one landed

| Capability | Competitors | Decision |
|---|---|---|
| Lowercase hex output | all 3 | **In model** — `output=hex`, the default |
| Base64 output | all 3 | **In model** — `output=base64` |
| Byte-array / `[]byte` view | ref45638 | **In model** — `output=bytes` emits a decimal `[130, 161, …]` array that pastes into `new Uint8Array(…)` or a Go/C literal |
| JSON-vs-binary byte count + % saving | openformatter, jsonutils | **In model** — `output=summary` and `output=json` both report `json_bytes`, `msgpack_bytes` and the saving percentage |
| Grouped / row-wrapped hex dump | jsonutils (16-byte rows) | **In model** — `group=N` inserts a space every N bytes; `group=16` reproduces the classic dump grouping |
| Annotated byte breakdown by type tag | openformatter (worked example only) | **In model, and better** — `output=annotated` produces a live per-value breakdown (offset, header bytes, tag name, meaning) for the payload the user actually pasted, not a static example |
| Loadable sample payload | openformatter, jsonutils | **In model** — three `[[example]]` preset chips (telemetry object, mixed array, annotated breakdown) prefill the form in one click |
| Copy / download the result | all 3 | **Platform** — the generated page ships copy + download for `format = "text"` pages |
| Clear JSON validation errors | all 3 | **In model** — `invalid JSON: …` with serde's line/column, plus explicit empty-input and size-cap messages |
| Client-side / no upload | all 3 | **In model** — the page runs the same Rust/WASM encoder locally; the CLI runs it on your machine |
| Deterministic key order | none | **In model, differentiator** — `key_order=sorted` sorts map keys by raw UTF-8 key bytes for byte-reproducible payloads; `input` (default) preserves document order like the competitors |
| float32 down-encoding when lossless | none (openformatter documents float64-only) | **In model, differentiator** — `compact_floats=true` writes float32 whenever the value round-trips exactly, which is where most of MessagePack's numeric saving actually comes from |
| Old-spec (pre-2013) string headers | none | **In model, differentiator** — `spec=old` skips the `str8` header so payloads stay readable by pre-2013 decoders; `new` (default) uses `str8` |

## Out of model — listed, not built

- **Binary (`bin`) and extension (`ext`) types, including the timestamp extension.** JSON has no
  byte-string or tagged-value type, so there is nothing in the input to map onto them. Decoding
  those types is already covered by the sibling `msgpack-to-json` tool.
- **Raw `.msgpack` binary file download.** The page renders `format = "text"`; the text output
  downloads fine, but emitting a raw binary attachment is a generator-level capability, not a
  block-level one. `output=base64` is the documented path to reconstruct the exact bytes.
- **Bidirectional decode in this tool.** Deliberately out of scope: the decode direction already
  exists as `msgpack-to-json`, and duplicating it here would make the two tools semantic dups.
- **File upload of a JSON document.** Pure text-in tool; the 1,000,000-byte paste cap is stated on
  the page.
- **Dark mode / theme switching.** Site chrome, injected by the consuming site repo, not by a block.

## UX controls adopted

- `output`, `key_order` and `spec` render as `<select>` menus with friendly `[input.labels]`.
- `compact_floats` renders as a checkbox (default off).
- `json` is `multiline = true` so pasted, pretty-printed documents keep their newlines.
- Three `[[example]]` preset chips stand in for the competitors' "Load Sample" buttons.

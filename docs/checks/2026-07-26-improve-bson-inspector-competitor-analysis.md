# bson-inspector — competitor analysis (2026-07-26)

Tool: parse a BSON document into a typed tree (element names, BSON types, nested
documents) and/or MongoDB Extended JSON. Browser-local wasm, no upload.

## Competitors scanned (paraphrased; no copy/branding reproduced)

1. **MongoDB `bsondump`** (official CLI, database-tools docs). Converts BSON files
   to human-readable output. Modes: `--type=json` (default) emits **Extended JSON
   v2.0 Canonical**, `--type=debug` emits a structural debug view; `--pretty`
   pretty-prints. `--objcheck` validates BSON before output (on by default).
   Represents BSON-specific types (ObjectId, dates, binary, int64, decimal) in
   Extended JSON form.
2. **jsontotable.org BSON→JSON**. Accepts BSON as **Base64 or Hex** ("from
   mongodump exports or MongoDB drivers"), plus `.bson`/`.bin` file upload and a
   sample loader. Preserves Mongo types (ObjectId, Date, Binary); "validates
   structure and reports integrity issues"; formatted/indented JSON output;
   copy + download.
3. **onlinetools.com BSON→JSON**. Output indentation choices: **spaces (count
   configurable), tab, or minify**. File import, save/download, copy, URL
   `?input=` param prefill, clickable examples.

(4th/5th — mcraiha BSON-hex→JSON JS tool and MongoDB's own Extended-JSON docs —
confirm the same shape: hex/base64 in, Extended-JSON-typed out.)

## Table-stakes params / defaults / UX (tagged in-model / out-of-model)

| Capability | Competitor | Our decision |
|---|---|---|
| Input as **Base64** | jsontotable, bsondump-file | in-model — `input_format=base64` (default) |
| Input as **Hex** | jsontotable | in-model — `input_format=hex` |
| **Extended JSON** output (canonical v2) | bsondump default | in-model — `output=json` (canonical Extended JSON v2) |
| **Typed structural / tree** view | bsondump `--type=debug` | in-model — `output=tree` (default; our differentiator per backlog) |
| **Pretty-print / indent** (spaces count, minify) | onlinetools, bsondump `--pretty` | in-model — `indent` (0 = minified, default 2) |
| Preserve Mongo types (ObjectId, Date, Binary+subtype, int32/64, decimal128, timestamp, regex, code, min/maxKey) | all | in-model — full BSON type coverage |
| **Validate structure / report errors** | jsontotable "reports integrity issues"; bsondump `--objcheck` | in-model — strict parse, actionable errors (length/terminator/type/offset) |
| **Byte offsets** per element | bsondump debug-ish | in-model — `show_offsets` (tree mode) |
| Example presets / sample loader | jsontotable, onlinetools | in-model — `[[example]]` chips |
| `?input=` URL prefill | onlinetools | in-model — native query-param deep-links |
| Copy / Download output | all | in-model (generic) — page `format=text` gets a Download link + copy is platform chrome |
| **File upload** (.bson/.bin) | jsontotable, onlinetools | out-of-model — pure page is a paste field; users paste base64/hex (mongodump/driver output). Listed, not built |
| Pastebin export / tool-chaining | onlinetools | out-of-model — no backend |
| Accounts / paid tiers / daily limits | onlinetools premium | out-of-model — gizza is free + local |

## Design decisions

- `output=tree` is the default (backlog asks for the typed tree); `output=json`
  emits **canonical Extended JSON v2** matching `bsondump` (`$oid`, `$date`:
  `{$numberLong}`, `$binary`:`{base64,subType}`, `$numberInt`, `$numberLong`,
  `$numberDouble`, `$numberDecimal`, `$timestamp`, `$regularExpression`, `$code`,
  `$symbol`, `$dbPointer`, `$undefined`, `$minKey`/`$maxKey`).
- `indent` (0–8, default 2) covers the pretty/minify axis; 0 = minified.
- Decimal128 rendered via the General-Decimal-Arithmetic to-scientific-string
  (matches MongoDB canonical `$numberDecimal`), Infinity/NaN preserved.
- Document field ORDER is preserved (BSON is ordered) — JSON is hand-serialized,
  not through a key-sorting map.

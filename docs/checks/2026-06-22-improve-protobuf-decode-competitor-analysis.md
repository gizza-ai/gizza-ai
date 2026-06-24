# protobuf-decode — competitor analysis (2026-06-22)

Tool: `gizza-ai/protobuf-decode` — decode raw Protocol Buffers wire bytes
(base64 or hex) into a field-number / wire-type tree, **without a `.proto`
schema**, recursing into nested length-delimited messages.

## Surfaces verified

- **Chat block**: `wafer build` validates + instantiates the wasm32-wasip1
  block (339.6 KiB, OK). Pure compute, no host imports.
- **CLI**: `gizza tool protobuf-decode input="08 96 01" encoding=hex` →
  field tree; `format=text` + nested `1a03089601` → indented `message { ... }`.
- **Page** (`/tools/protobuf-decode/`): 4 Playwright tests pass — hex default
  json, text-format nested recursion, base64 input, and query-param deep-link
  (`?input=…&format=text`).
- Unit tests: 10 core + 1 drift-guard schema test, all passing.

## Top competitors surveyed

1. **protobuf-decoder (pawitp, protobuf-decoder.netlify.app)** — the most-used
   free schemaless decoder. Paste hex; it shows a collapsible field tree, lets
   you toggle each field's interpretation (varint as int / sint / the
   length-delimited blob as message / string / bytes), and auto-detects nested
   messages.
2. **ProtobufPal (protobufpal.com)** — online encode + decode; decode needs a
   pasted `.proto` (schema-driven) but also has a no-schema raw view.
3. **protoscope (Google, github.com/protocolbuffers/protoscope)** — CLI; emits a
   text disassembly of the wire format; the reference for "interpret bytes
   without a schema".
4. **`protoc --decode_raw`** — the canonical schemaless decoder shipped with
   protobuf; reads binary from stdin, prints a field-number/value outline.
5. **Various "protobuf viewer" web apps / VS Code extensions** — most require a
   schema; the schemaless ones mirror pawitp's tree.

## Feature diff & gaps (fit-to-model)

| Capability | Competitors | gizza protobuf-decode | Action |
|---|---|---|---|
| Schemaless decode (field# + wire type) | all | yes | covered |
| Hex input | all | yes | covered |
| Base64 input | pawitp, protobufpal | yes (+ URL-safe, padless) | covered |
| Auto-detect hex vs base64 | few | yes (`encoding=auto` default) | **edge** |
| Recurse into nested messages | pawitp, protoscope | yes | covered |
| Show multiple interpretations per field | pawitp | yes (uint/int/sint/bool; u32/i32/float; u64/i64/double; string/bytes/message) | covered |
| zigzag (sint) interpretation | pawitp (toggle) | yes (always shown) | covered |
| float/double interpretation of fixed fields | pawitp | yes | covered |
| Structured JSON output (machine-readable) | rare | yes (`format=json` default) | **edge** — most only render an HTML tree |
| Compact text outline | protoscope, protoc | yes (`format=text`) | covered |
| Privacy / runs locally | netlify ones are client-side | yes (WASM, no server) | covered |
| **Schema-driven decode (paste a `.proto`)** | protobufpal, protoc | **NO** | OUT OF MODEL — needs a `.proto` parser + the multi-line schema as a second input; large scope. Deferred. |
| **Interactive per-field interpretation toggles** | pawitp | partial — we emit ALL interpretations at once instead of a UI toggle | acceptable (static page model; richer than a single guess) |
| **Encode (build wire bytes from a tree)** | protobufpal | NO | OUT OF MODEL — separate (inverse) tool; not "decode". |
| **File upload of a `.bin`** | some | NO (paste base64/hex) | acceptable — text input keeps all 3 surfaces working; binary file-in would lose the page text path. |

## Gaps closed this run

The initial scaffold already shipped the core capability set above. Versus the
leading free competitor (pawitp), the in-model edges we match-or-beat:

- **All interpretations emitted at once** rather than a manual per-field toggle
  — a single response is fully informative for an LLM and for copy/paste.
- **`format=json`** gives a machine-readable tree (most web tools only render
  HTML), so the output can feed another gizza tool.
- **`encoding=auto`** removes the "is this hex or base64?" friction.

## Out-of-model (intentionally not built)

- **Schema-driven decode** (paste a `.proto`): requires a full proto-IDL parser
  and a second multi-line input; large and orthogonal to the schemaless value
  prop. Deferred.
- **Encoding** (tree → wire bytes): the inverse tool, not a decoder.
- **Group wire types 3/4**: deprecated in proto3; we report a clear error
  rather than silently mis-parsing.

No competitor copy, branding, or trademarks were reused.

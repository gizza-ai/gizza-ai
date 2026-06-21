# base32-codec — competitor analysis (2026-06-21)

Tool: **Base32 Encoder / Decoder** (`blocks/base32-codec`). Encodes text or bytes
to RFC 4648 Base32 and decodes Base32 back, with the base32hex, Crockford, and
z-base-32 variants, hex byte I/O, and optional lowercase/padding.

## Surfaces verified

- **Chat / LLM API** — descriptor-derived schema (6 params: `input`, `mode`,
  `variant`, `format`, `lowercase`, `padding`); drift-guard schema test passes.
  `wafer build` instantiates the block.wasm (305.5 KiB).
- **CLI** — `gizza tool base32-codec ...` verified across encode/decode, all four
  variants, hex I/O, unpadded, and the undecodable-input error path.
- **Page** — `/tools/base32-codec/` renders the textarea, three `<select>`s, and
  two checkboxes; 7 Playwright tests pass incl. the `?input=&variant=` deep-link.
- **Core** — 14 unit tests, anchored on the RFC 4648 §10 standard and base32hex
  test vectors (`""`, `f`, `fo`, `foo`, `foob`, `fooba`, `foobar`).

## Top competitor tools surveyed

(Surveyed for capability/UX surface, not for copy — no competitor wording or
branding was reused.)

1. **cryptii.com** — modular conversion pipeline incl. Base32. Strength: arbitrary
   byte input via a bytes view, chained conversions. Generic, not a focused Base32
   page; no Crockford/z-base-32 presets.
2. **base64.guru / base32 section** — encode/decode with standard alphabet,
   file upload. Strength: file input. Weakness: standard alphabet only, ad-heavy.
3. **emn178 online tools (base32)** — encode/decode, hex output toggle, UTF-8 vs
   raw. Strength: hex output view. No Crockford / z-base-32.
4. **dcode.fr Base32 cipher** — supports several alphabets including a custom one
   and explains the encoding. Strength: alphabet variety + explanation. Weakness:
   French-first UI, puzzle-oriented, no padding control.
5. **devToolbox / IT-Tools "Base32" + "Crockford"** — separate widgets for RFC 4648
   and a dedicated Crockford encoder. Strength: Crockford support. Split across two
   widgets; no z-base-32, no hex byte input.

## Gap analysis vs. our build

| Capability | Competitors | base32-codec |
| --- | --- | --- |
| Standard RFC 4648 encode/decode | all | yes (default) |
| base32hex (RFC 4648 §7) | emn178/dcode partial | yes |
| Crockford Base32 | IT-Tools, dcode | yes |
| z-base-32 | rare | yes |
| Hex byte input/output (binary data) | emn178 (output only) | yes (both directions) |
| Optional `=` padding toggle | rare | yes |
| Lowercase output | rare | yes |
| Case-insensitive / padding-tolerant decode | varies | yes |
| Runs fully client-side, no upload, offline | some | yes |
| File upload of binary | base64.guru | no (hex byte I/O covers binary text-side) |
| Chained/multi-step pipeline | cryptii | out of scope (single-purpose tool) |

## In-model gaps closed

- All four common alphabets in one tool (most competitors cover 1–2): standard,
  base32hex, Crockford, z-base-32.
- Binary data handled without a file upload via the **hex** data format on both
  encode and decode — covers the "decoded output is garbled / not UTF-8" case that
  trips up text-only competitors.
- Explicit **padding** and **lowercase** toggles, plus padding-tolerant,
  case-insensitive decoding.

## Out-of-model / deliberately not built

- **Binary file upload** (drag-and-drop a file to encode its bytes): the page's
  single text field + the chat/CLI string contract don't carry a binary file
  input; hex byte I/O is the supported path for binary data. Matches the platform's
  existing text-tool shape.
- **Multi-step conversion pipeline** (cryptii-style chaining): out of scope for a
  focused single-purpose tool.
- **Custom user-defined alphabets**: niche; the four standard variants cover the
  real-world uses (TOTP, DNSSEC NSEC3, human IDs).

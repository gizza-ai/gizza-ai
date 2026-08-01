# html-entity-encoder — competitor analysis (2026-07-30)

New tool built end-to-end. Complements the existing `html-entity-decoder` (the reverse
operation) and is richer than `string-escaper`'s minimal 5-char HTML mode. Scan run before
implementing. All notes are **paraphrased** — no competitor copy, branding, or trademarks
reproduced.

## Search

Query: "HTML entity encoder online tool encode special characters named numeric". Picked the
top real, reachable tools. inventivehq (403 to the fetcher) covered via the search snippet.

## Competitor profiles (paraphrased)

### DevBolt — HTML Entity Encoder/Decoder
- **Scope modes:** Minimal (default) = the five HTML/XML-sensitive chars (`& < > " '`);
  "All Characters" = also encodes non-ASCII (accents, symbols, emoji), producing pure-ASCII
  output, named entities where they exist and numeric codes otherwise.
- **Output formats:** named, decimal, hex.
- **UX:** mode toggle, Encode/Decode buttons (Ctrl+Enter), two-panel input/output.
- **Worked example (paraphrased):** an HTML `<div>` snippet in minimal mode returns the five
  sensitive characters escaped, tags and quotes turned into `&lt;`/`&gt;`/`&quot;`/`&amp;`.

### hidekazu-konishi — HTML Entity Encoder/Decoder
- **Three scope modes:** "Safe Minimum" (the five chars); "All Special Characters" (every char
  that has a named entity); "Full Non-ASCII" (every char above ASCII).
- **Output formats:** named / decimal / hex, chosen via radio/toggle.
- **UX:** two-panel layout; format + scope selectors; a searchable/sortable 250+ named-entity
  reference table grouped by category; a client-side "character analysis" breakdown.

### Inventive HQ — HTML Entity Encoder (from search snippet; page 403'd the fetcher)
- Named and numeric entities; free, client-side XSS-prevention framing.

### W3docs — HTML Encoder/Decoder
- Four actions (encode/decode entities, encode/decode tags); can turn every printable char into
  a decimal numeric entity.

## Table-stakes → design decisions

| Capability | Competitors | Our decision | In/out of model |
| --- | --- | --- | --- |
| Scope: minimal (5 chars) | all | `scope=minimal` (default) | **in** |
| Scope: all non-ASCII | devbolt, hidekazu | `scope=non-ascii` | **in** |
| Scope: everything with a named entity | hidekazu | `scope=named` | **in** |
| Output format named/decimal/hex | all | `format` enum (default named) | **in** |
| Named where available, numeric fallback | devbolt, hidekazu | named format falls back to decimal | **in** |
| Pure client-side, offline | all | wasm, no network | **in** (already) |
| Worked examples / presets | all | `[[example]]` preset chips | **in** |
| Reference table of common entities | hidekazu | static table in `content.md` | **in** |
| Decode direction | all bundle it | already shipped: `html-entity-decoder` | **in** (separate tool) |
| Encode/decode-tags-only actions (W3docs) | w3docs | not built — niche; scope/format covers the real need | considered, rejected |
| Live per-character analysis panel, sortable searchable table | hidekazu | not built — bespoke JS UI; the static reference table + worked examples cover it | out-of-model (UI-only, big) |
| Server/API/batch, accounts | — | n/a — gizza is browser-local | out-of-model |

## Orthogonal design

Two independent axes, matching the strongest competitors:
- **scope** = *which* characters get encoded: `minimal` | `non-ascii` | `named`.
- **format** = *how* each encoded character is represented: `named` (falls back to decimal when
  no name exists) | `decimal` (`&#NNN;`) | `hex` (`&#xHH;`).

The five HTML/XML-mandatory characters (`& < > " '`) are always in every scope. Named format
uses the HTML5 named set (e.g. `&amp;`, `&copy;`, `&mdash;`); the apostrophe becomes `&apos;`
(an HTML5 name) — decimal/hex give the HTML4-safe `&#39;`/`&#x27;` when needed.

Spike: confirmed the `entities` crate (already used by the decoder) exposes the full HTML5 set
as a flat `ENTITIES` array (`entity`, `codepoints`, `characters`), enough to build a
char → shortest-named-entity map for encoding — no new dependency and no out-of-model gap.

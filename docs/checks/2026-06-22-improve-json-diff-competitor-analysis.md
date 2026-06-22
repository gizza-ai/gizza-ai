# json-diff — competitor analysis (2026-06-22)

## Tool

`blocks/json-diff` — structural diff of two JSON documents. Compares objects
key-by-key and arrays index-by-index, recursively, and returns a machine-readable
JSON report: `{ equal, added, removed, changed, changes:[{ path, kind, old?, new? }] }`
where each `path` is a JSON path (`$.a.b`, `$.list[2]`). Pure-Rust (serde_json),
runs on all surfaces (chat / CLI / page), nothing uploaded.

## Top competitors surveyed

1. **jsondiff.com** — the canonical "semantic JSON compare". Paste two blobs, see
   added/removed/modified keys in a side-by-side nested view.
2. **jsondiffpatch (online demo)** — open-source library; computes a structural
   delta/patch with a compact nested view.
3. **SemanticDiff (online JSON diff)** — ignores whitespace/formatting, highlights
   only key/value changes using a representation-independent structure.
4. **Data Formatter Pro — JSON Compare** — deep structural compare with three
   views: side-by-side, inline (git-style), and unified-by-path. Browser-only.
5. **TextCompare.org JSON tool** — red/green added/deleted highlighting, counts of
   added/deleted, save-as-PDF, share-via-URL. Browser-only.

Sources:
- [JSON Diff — jsondiff.com](https://jsondiff.com/)
- [JSON Compare — jsoncompare.org](https://jsoncompare.org/)
- [Top 5 JSON compare tools — dataformatterpro.com](https://dataformatterpro.com/blog/top-5-websites-tools-compare-json/)
- [SemanticDiff online JSON diff](https://semanticdiff.com/online-diff/json/)
- [TextCompare.org JSON](https://www.textcompare.org/json/)

## Capability diff (competitors vs gizza json-diff)

| Capability | Competitors | gizza json-diff | Status |
|---|---|---|---|
| Deep/recursive structural compare | yes | yes | met |
| Added / removed / changed classification | yes | yes (`kind`) | met |
| Per-location JSON path | DataFormatterPro (unified-by-path) | yes (`$.a.b`, `$[2]`) | met |
| Old + new value per change | most | yes (`old`/`new`) | met |
| Equal/identical signal | implicit | yes (`equal` + counts) | met |
| Privacy / browser-only | most | yes (wasm, nothing uploaded) | met |
| Machine-readable diff output | jsondiffpatch (delta) | yes (JSON report) | met |
| Configurable / minified output | rare | yes (`indent`, 0=minify) | met (edge) |
| Side-by-side visual rendering | yes | no (returns structured JSON) | out of model |
| Inline / git-style colored diff | yes | no | out of model |
| Whitespace/format-ignoring | SemanticDiff | yes (parses to values, formatting ignored inherently) | met |
| Ignore-key rules / custom matchers | a few | no | out of model (gap) |
| Save-as-PDF / share-via-URL | TextCompare | no | out of model |

## Gaps assessed (fit-to-model)

In-model gaps closed / already covered:
- **JSON-path reporting** — implemented (`$.a.b[2]` style); this is the
  unified-by-path view competitors charge for, exposed as data.
- **Equal signal + counts** — `equal`, `added`, `removed`, `changed` summary added
  so callers/LLMs can branch on "are these the same?" without scanning the array.
- **Whitespace/formatting independence** — inherent: both inputs are parsed to
  `serde_json::Value`, so formatting/key-order-on-the-wire never produces noise
  (object key compare is by key, not position).
- **Minify/indent control** — `indent` param (0 = minified, default 2).

Out-of-model (NOT built — gizza tools return data, not rendered UI):
- Side-by-side / inline colored visual diff rendering (presentation layer; the
  page shows the JSON report, which is the gizza tool contract).
- Save-as-PDF, share-via-URL (page-platform features, not per-tool).
- Custom ignore-key rules / fuzzy array matching by id — a possible future
  enhancement but a larger semantic-matching design; left out to keep the tool a
  deterministic, predictable structural diff. Noted, not built.

## Verification (3 surfaces)

- **Chat block**: `wafer build` — OK, `target/block.wasm` validated (341.8 KiB).
- **CLI**: `gizza tool json-diff left=… right=…` — returns the expected report
  (changed `$.age`, changed `$.tags[1]`, added `$.city`).
- **Page**: Playwright `tool-page-json-diff.spec.ts` — 2/2 passing (changed+added
  minified report; equal-document report).
- Unit tests: 11 core + 1 drift-guard schema test, all passing.

No trademarks, branding, or copy were taken from any competitor.

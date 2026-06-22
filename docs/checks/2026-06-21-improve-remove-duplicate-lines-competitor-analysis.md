# remove-duplicate-lines — competitor analysis (2026-06-21)

Pure-Rust, in-browser/CLI tool that removes duplicate lines from text and returns
the deduplicated text plus total/kept/removed line counts. Three surfaces verified:
chat block (`wafer build` OK), CLI (`gizza tool remove-duplicate-lines …`), and the
page (Playwright 3/3).

## Top competitors surveyed

1. **miniwebtool — Remove Duplicate Lines** (https://miniwebtool.com/remove-duplicate-lines/) —
   keep first / keep last / remove-all modes, case sensitivity, trim whitespace,
   statistics (lines processed, duplicates found, compression ratio).
2. **lazytools.io — Remove Duplicate Lines** (https://lazytools.io/remove-duplicate-lines/) —
   case-insensitive, trim-before-compare, keep-last mode, a panel of which lines were
   removed, plus output sorting (A→Z, Z→A, by length, natural numeric).
3. **listwrangler.app — Remove Duplicates** (https://listwrangler.app/remove-duplicates/) —
   case sensitive/insensitive, keep first or last.
4. **texttoolsuite.com — Remove Duplicates** (https://texttoolsuite.com/remove-duplicates/) —
   trim / case / whitespace / Unicode normalization, keep first or last, preserve order
   or sort, export.
5. **text-format.com — Remove Duplicate Lines** (https://text-format.com/removeduplicates) —
   basic in-browser dedupe, preserve order.

## Capability diff (gizza vs. field)

| Feature | Competitors | gizza remove-duplicate-lines |
|---|---|---|
| Keep first occurrence (order preserved) | yes | yes (default) |
| Keep last occurrence | most | yes (`keep=last`) |
| Case-insensitive matching | yes | yes (`ignore_case`) |
| Trim whitespace before compare | yes | yes (`trim`, also trims kept output) |
| Remove blank/empty lines | some | yes (`remove_empty`) |
| Consecutive-only collapse (`uniq`) | rare | **yes (`adjacent_only`)** — differentiator |
| Removed/kept statistics | miniwebtool | yes (total/kept/removed in chat + CLI JSON) |
| Runs locally / nothing uploaded | yes (JS) | yes (WebAssembly) |
| CLI + LLM/chat API surface | no | **yes** — differentiator |

## Gaps deliberately NOT built (scope / separate tool)

- **Output sorting (A→Z, Z→A, by length, natural numeric).** In-model but a distinct
  concern — gizza keeps dedupe and sorting as separate tools (a sort/list tool already
  exists). Dedupe here is strictly order-preserving, which is the safer default.
- **"Removed lines" diff panel (page UI).** The counts (total/kept/removed) are returned
  on the chat/CLI surfaces; a visual removed-lines panel is a page-only UX nicety, out of
  scope for the shared-core model (the page renders a single text output).
- **Unicode normalization (NFC/NFD) before compare.** Niche; not added to avoid surprising
  byte-level changes to kept lines.

## Notes

No competitor copy, branding, or trademarks were used. Tool copy and behavior are original.
The consecutive-only (`uniq`) mode and the CLI/chat-API surfaces are the main advantages
over the surveyed JS-only web tools.

Sources:
- https://miniwebtool.com/remove-duplicate-lines/
- https://lazytools.io/remove-duplicate-lines/
- https://listwrangler.app/remove-duplicates/
- https://texttoolsuite.com/remove-duplicates/
- https://text-format.com/removeduplicates

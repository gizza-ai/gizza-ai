# list-dedupe-merge — competitor analysis (2026-07-25)

Tool: **list-dedupe-merge** — "Merges two lists into one deduplicated list and reports how many
overlapping entries were collapsed." Pure, runs client-side in wasm.

## Competitors scanned

1. **usefmtly — Merge Lists** (`usefmtly.com/tools/list-tools/merge-lists/`) — four modes
   (concatenate / interleave / union / intersection); trim, remove-empty, remove-duplicates,
   sort A–Z, case-sensitive toggle (default case-insensitive); three-column live view with a
   per-column item count and a "Dupes rm" counter; copy button.
2. **alotoftools — List Merger** (`alotoftools.com/list-merger`) — modes append / interleave / zip
   / alternate-lines; **separate input and output separators**; remove-duplicates, remove-empty,
   trim checkboxes; Merge / Swap / Copy / Clear buttons; per-input item counters.
3. **gillmeister — Merge lists line by line** — merges 2–5 lists line-by-line with per-list
   prefix/suffix and an optional delimiter; **no** dedupe, sort, case, or duplicate reporting
   (a line-zipper, not a dedupe merger).
4. **Browserling — Merge Two Lists** (reachable, skimmed) — bare "Concatenate Lists" of two
   line-lists; no dedupe/case options.

(usefmtly is the closest functional match; gillmeister is the weakest — a line-zipper without any
dedupe, kept only to show the low end.)

## Table-stakes → decision (in-model / out-of-model)

| Capability | Competitors | Decision |
|---|---|---|
| Two list inputs, one item per line | all | **in-model** — `list_a`, `list_b` |
| Deduplicate merged output (core purpose) | usefmtly, alotoftools | **in-model** — always on (this IS the tool); union semantics |
| Report count of duplicates / overlap removed | usefmtly ("Dupes rm") | **in-model** — summary line reports `duplicates removed` + `shared by both` (cross-list overlap), the tool's headline number |
| Merge order: append (concat) vs interleave | usefmtly, alotoftools | **in-model** — `merge_order` = append (default) / interleave |
| Separator (newline/comma/tab/semicolon/pipe/space) | alotoftools | **in-model** — `separator` enum splits both inputs |
| Trim whitespace | all | **in-model** — `trim` (default true) |
| Remove blank/empty items | usefmtly, alotoftools | **in-model** — `ignore_blank` (default true) |
| Case-insensitive matching | usefmtly | **in-model** — `ignore_case` (default false) |
| Sort result A–Z / Z–A | usefmtly | **in-model** — `sort` = input/asc/desc |
| Ignore leading zeros (numeric IDs) | (ours, extends set-diff family) | **in-model** — `ignore_leading_zeros` (default false) |
| Preset example chips | (common UX) | **in-model** — `[[example]]` chips |
| Per-column live item counters | usefmtly, alotoftools | **out-of-model** — a live-editor UI affordance; ours reports A/B/merged counts in the summary line instead |
| Separate *output* separator | alotoftools | **out-of-model** — merged list is newline-joined for readability alongside the summary; noted as a limit |
| Zip / alternate-lines / prefix-suffix line templating | alotoftools, gillmeister | **out-of-model** — those are line-templating (gillmeister) not dedupe-merge; out of scope for a dedupe tool |
| Intersection mode | usefmtly | **out-of-model** — that's the existing `list-set-diff` tool (only-A / only-B / shared); this tool is the union/merge complement |
| Copy / Swap / Clear buttons | usefmtly, alotoftools | **out-of-model** — generic page-shell affordances, not tool params |

Every table-stake above is either in the descriptor or explicitly listed out-of-model; none dropped
silently. No competitor copy, branding, or trademarks were reproduced — behaviours paraphrased only.

## Relationship to existing tools

- `list-set-diff` — the **difference** view (only-A / only-B / shared). This tool is the
  complementary **union/merge** view (one combined deduplicated list + overlap count). Distinct
  user intent; not a duplicate.
- `email-list-cleaner`, `csv-dedupe`, `fuzzy-dedupe` — single-list or format-specific dedupe; none
  merge two lists and report cross-list overlap.

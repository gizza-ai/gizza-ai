# find-unique-lines — competitor analysis (2026-06-22)

## What this tool does

Returns only the lines that appear **exactly once** across the input — the
classic `uniq -u` behaviour. Lines that repeat are dropped entirely. This is
**not** deduplication (keeping one copy of each line); it is *one-off
detection*. Results are returned in first-appearance order, with options for
case-insensitive and whitespace-trim matching, plus total/distinct line counts.

## Surfaces verified

- **Chat / LLM API** — `descriptor()` single-sources the schema; drift-guard unit
  test (`schema_json_matches_authored_chat_schema`) passes. 8 core unit tests pass.
- **CLI** — `gizza tool find-unique-lines text=$'apple\nbanana\napple\ncherry\nbanana\ndate'`
  → `{"distinct_lines":4,"total_lines":6,"unique_count":2,"unique_lines":["cherry","date"]}`.
  `ignore_case=true` on `Foo\nfoo\nbar` → `["bar"]`. Both correct.
- **Page** — `/tools/find-unique-lines/`, 2 Playwright tests pass (one-off filtering
  + ignore-case). Runs in-browser via wasm; nothing uploaded.

## Competitor landscape

The closest public tools are "remove duplicate lines / find unique lines" online
utilities. Notably most of them conflate "unique" with "deduplicate" (keep one
copy of each line), which is a **different** operation from `uniq -u`.

| Competitor | Core feature set | Notes |
|---|---|---|
| [Text Tools — Remove Duplicate Lines](https://texttools.org/remove-duplicate-lines) | dedupe; trim toggle; case-insensitive toggle | "find unique lines … delete the copies" = dedupe semantics |
| [PicoToolkit — Remove Duplicates](https://picotoolkit.com/text/remove-duplicates) | dedupe; case toggle; trim; strip prefix/suffix; count removed | strip-affix is extra, but still dedupe |
| [GPT Cleanup](https://www.gptcleanup.com/remove-duplicate-lines) | dedupe; keep first; case-insensitive; trim; preserve order | preserves first-appearance order |
| [FatAIM Tools](https://fataimtools.com/converter-tools/remove-duplicate-lines/) | dedupe; keep first/last; case toggle; trim; keep order | keep-first/last option |
| [Online Text Tools — Remove Duplicate Lines](https://onlinetexttools.com/remove-duplicate-text-lines) | has a mode that "removes all lines that occur two or more times, leaving only absolutely unique lines" | this matches our `uniq -u` semantic |

## Gap analysis (fit-to-model)

Capabilities common across competitors, mapped to this tool:

- **Case-insensitive matching** — IMPLEMENTED (`ignore_case`).
- **Trim whitespace before comparing** — IMPLEMENTED (`trim`).
- **Preserve original (first-appearance) order** — IMPLEMENTED (default and only
  behaviour; covered by a unit test).
- **Report counts** — IMPLEMENTED (`total_lines`, `distinct_lines`,
  `unique_count` in the structured chat/CLI output).
- **Privacy / in-browser** — IMPLEMENTED (pure wasm; the page states nothing is
  uploaded).

Differentiator we lead on: we cleanly implement the **exactly-once** (`uniq -u`)
semantic and label it as distinct from deduplication, which most competitors
blur. The page copy explicitly contrasts the two so users pick the right tool.

### Considered and deliberately NOT added

- **Strip common prefix/suffix before comparing** (PicoToolkit) — niche; would
  change the comparison key in surprising ways and overlaps poorly with a
  one-off finder. Skipped to keep the tool focused.
- **Keep first vs. last occurrence** (FatAIM) — meaningless for `uniq -u`: a
  unique line has exactly one occurrence, so there is no first/last choice.
- **Sort output** — competitors that sort lose first-appearance order; we keep
  order, which is the more useful default and matches GPT Cleanup / FatAIM.

No competitor copy, branding, or trademarks were used.

## Result

All in-model gaps were already closed by the initial build; the tool matches or
exceeds the common competitor feature set for the exactly-once use case. No
follow-up changes were required.

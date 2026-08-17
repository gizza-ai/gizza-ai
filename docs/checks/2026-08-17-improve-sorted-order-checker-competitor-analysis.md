# sorted-order-checker — competitor analysis (2026-08-17)

Scan run BEFORE implementing, per `create-next-tool` step 4. Everything below is a paraphrase of
observed behaviour; no competitor copy, branding, or trademark is reused anywhere in this block.

Backlog row: `sorted-order-checker` — "Verifies a numeric list is sorted ascending or descending and
pinpoints the first out-of-order element." (type_hint `pure`).

## Duplicate check

`ls blocks/ | grep -iE 'sort|order|sequen|monoton|list|numeric'` returns sorters and validators, none
of which answer "is this list already in order?":

| Existing block | What it does | Why not a duplicate |
| --- | --- | --- |
| `csv-sort`, `sort-lines`, `sort-json-array`, `json-sort`, `file-list-sorter` | **Re-order** rows/lines/arrays | They mutate the list; they never report whether the input was already ordered, nor where it breaks |
| `numeric-range-check` | Flags CSV cells outside a min/max range | Per-cell bounds check, no relationship between neighbouring values |
| `numeric-row-deduplicator`, `list-dedupe-merge` | Duplicate removal | Equality, not ordering |
| `descriptive-stats`, `csv-stats`, `moving-average` | Aggregate statistics | No monotonicity verdict |

No skiplist entry points at this slug. Building.

## Competitors reviewed (top 3)

1. **Number Sorter (miniwebtool)** — https://miniwebtool.com/sort-numbers/
   Accepts numbers separated by commas, spaces, or line breaks; handles integers, decimals,
   negatives, and values written with thousands separators (`1,234,567`); documents a ~10,000-number
   working size. Ascending/descending radio buttons, three output-separator choices (line break,
   comma, space), a copy button, worked "example" buttons, and a statistics block (count, min, max,
   sum, mean, median, range, standard deviation, sign counts, integer-vs-decimal split) plus a
   distribution histogram.
2. **Sort a List of Numbers / Sort a List (onlinetools.com, by Browserling)** —
   https://onlinetools.com/number/sort-numbers , https://onlinetools.com/list/sort-list
   Ascending/descending order; a user-specified **input separator** and a separate **output
   separator** (defaults line break; the list variant also allows a regex separator); "skip duplicate
   numbers"; trim/remove-empty-item options on the list variant; runs entirely client-side; copy /
   download / save buttons; an examples gallery.
3. **"Check if an array is sorted" reference implementations** — GeeksforGeeks
   (https://www.geeksforgeeks.org/dsa/program-check-array-sorted-not-iterative-recursive/) and the
   equivalents in Python (`all(a <= b for a, b in pairwise(x))`, `pandas.Index.is_monotonic_increasing`)
   and JS. These are what people actually reach for when they want a *verdict* rather than a sort.
   Semantics they establish, and which this tool must match: sortedness is a pairwise neighbour check
   (`arr[i-1] <= arr[i]`); **equal adjacent values count as sorted** by default (non-strict), so a
   strict/unique variant has to be an explicit option; empty and single-element lists are sorted by
   definition; the scan stops at the **first violating pair**, which is exactly the "pinpoint the
   first out-of-order element" the backlog row asks for — and which none of the reference snippets
   actually *report* (they return a bare boolean).

## Table stakes → decisions

| Table stake | Seen at | Verdict | Where it landed |
| --- | --- | --- | --- |
| Comma / space / newline separated input | 1, 2 | in-model | `separator = auto` splits on commas, whitespace, newlines, semicolons and pipes; explicit `comma`/`newline`/`space`/`semicolon`/`tab`/`pipe` choices for ambiguous data |
| Explicit input separator choice | 2 | in-model | `separator` (`Param::enumv`) |
| Integers, decimals, negatives, scientific notation | 1, 2 | in-model | full `f64` parse (`-2.5`, `+3`, `.5`, `1e6`, `inf`); `NaN` is rejected as unorderable |
| Thousands separators inside values (`1,234,567`) | 1 | in-model | `strip_thousands` — when on, `,` and `_` inside a token are group separators, so `auto` stops splitting on commas (an explicit `separator = comma` with it on is a clear error, not silent nonsense) |
| Ascending **and** descending | 1, 2, 3 | in-model | `order = auto \| ascending \| descending`; `auto` detects the direction from the first differing pair and then verifies the whole list against it |
| Duplicates / strict-vs-non-strict ordering | 2 ("skip duplicates"), 3 (equal values count as sorted) | in-model | `strict` boolean — off = ties allowed (the reference default), on = strictly increasing/decreasing so repeated values are reported; equal adjacent pairs are counted in every report |
| First violating pair reported | 3 (the algorithm; none of them print it) | in-model — **our differentiator** | `First out-of-order element: position N, value V (previous position N-1, value P)` + a plain-English reason |
| All violations, not just the first | — (gap in all three) | in-model | full break list, capped by `max_issues` (default 20, `… and N more`) |
| Longest already-sorted run | — (gap) | in-model | reported for both verdicts — tells you how far the good prefix goes |
| Machine-readable output | 2 (download), 3 (boolean return) | in-model | `format = json` emits the whole report (verdict, direction, every break, run, min/max) for scripting |
| Copy / download / reset / deep-link | 1, 2 | in-model, platform-provided | the generator gives Copy result + Reset + Download on `format = "text"` pages, and `?param=` deep links |
| Worked example presets | 1, 2 | in-model | five `[[example]]` chips (ascending pass, find-the-break, strictly descending, one-per-line timestamps, thousands separators) |
| Descriptive statistics (sum, mean, median, stdev, sign counts) | 1 | **out of model — deliberately not built** | that is `blocks/descriptive-stats`' job; duplicating it here would make two pages compete for the same query. The report carries only the ordering-relevant figures (count, comparisons, min, max, first, last, equal-adjacent pairs) |
| Distribution histogram / charts | 1 | **out of model** | this repo's page driver renders text or media output, not client-side charts; a chart would also not change the sorted/not-sorted verdict |
| Actually sorting the list, output separators, dedupe | 1, 2 | **out of model (by design)** | this tool is a verifier, not a sorter — `blocks/sort-lines`, `blocks/csv-sort`, `blocks/sort-json-array` and `blocks/list-dedupe-merge` already own those. The report says where the order breaks so you can fix the source |
| Regex input separator | 2 | **out of model** | a fixed separator enum plus `auto` covers pasted numeric lists; a regex box on a numbers-only tool is a footgun with no observed demand |
| Import from file / export to paste services | 2 | **out of model** | paste-and-check is the whole interaction; the platform Download link covers getting the report out |

## Limits stated on the page

- 20,000 values per run (the exact boundary is Playwright-tested at 20,000 pass / 20,001 error).
- `max_issues` caps only how many breaks are *listed*; the totals always count every break.
- `NaN` is not orderable and is treated as a non-numeric token (`non_numeric = error | ignore`).
- Values are compared as `f64`, so integers beyond 2^53 may compare equal — stated in the FAQ.

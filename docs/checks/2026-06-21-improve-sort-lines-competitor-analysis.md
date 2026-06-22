# sort-lines — competitor analysis (2026-06-21)

Tool: `gizza-ai/sort-lines` — sort the lines of a block of text alphabetically,
numerically, naturally, or by length, ascending/descending, with optional
case-insensitivity, whitespace trimming, duplicate removal, and blank-line removal.

## Surfaces verified

- **Core unit tests** — 11 tests pass (`cargo test -p gizza-ai-sort-lines-core`).
- **Chat block** — `wafer build` validates + instantiates (359 KiB wasm); schema
  drift-guard test passes.
- **CLI** — `gizza tool sort-lines text=… method=… order=… unique=…` returns the
  expected sorted text + counts.
- **Page** — 3 Playwright tests pass (`tool-page-sort-lines.spec.ts`): alpha asc,
  numeric desc, natural + remove-duplicates.

## Top competitors surveyed

1. **onlinetexttools.com — Sort Text Lines** — alpha / numeric / by length, asc/desc,
   case-insensitive, optional dedup.
2. **miniwebtool.com — Sort Lines Alphabetically** — multiple sort options incl.
   natural sort, length, complexity; sort statistics visualization.
3. **gillmeister-software.com — Sort list online** — sort criterion + direction,
   numeric, natural, remove duplicates, sort by 1st/2nd word.
4. **limeconvert.com — Sort List** — alphabetical, natural, numeric sort, dedup.
5. **alllintools.com / text-tool.com — Text Line Sorter** — alphabetical & numerical,
   reverse, remove blanks, case handling.

## Feature diff (our tool vs. competitors)

| Capability | Competitors | sort-lines |
| --- | --- | --- |
| Alphabetical sort (A→Z / Z→A) | yes | **yes** (`alpha` + `order`) |
| Numeric sort (leading number) | yes | **yes** (`numeric`) |
| Natural sort (file2 < file10) | most | **yes** (`natural`) |
| Sort by line length | some | **yes** (`length`) |
| Ascending / descending | yes | **yes** (`order=asc|desc`) |
| Case-insensitive compare | yes | **yes** (`ignore_case`) |
| Ignore surrounding whitespace | some | **yes** (`trim`) |
| Remove duplicate lines | yes | **yes** (`unique`) |
| Remove blank lines | some | **yes** (`remove_blank`) |
| Privacy (runs locally, no upload) | claimed by some | **yes** (in-browser wasm) |

## In-model gaps closed

The initial design already covered the full common feature set found across the
top five competitors (alpha/numeric/natural/length, asc/desc, ignore-case, trim,
dedup, remove-blank). No further in-model capability gap was identified after the
survey, so no additional parameters were added beyond the original scope.

## Out-of-model / deliberately omitted

- **Random shuffle / "randomize order"** — non-deterministic; it does not fit the
  page's recompute-on-input model and is a different tool (a shuffler), not a sorter.
- **Sort by Nth word / column / email-domain** — a column/key selector is a
  meaningfully larger feature; deferred as a candidate for a separate `sort-by-column`
  tool rather than overloading this one.
- **Sort statistics visualization** — UI chrome outside the pure-compute tool model;
  the CLI/chat surfaces already return `total`, `lines`, and `removed_duplicates`.

No competitor copy, branding, or trademarks were used.

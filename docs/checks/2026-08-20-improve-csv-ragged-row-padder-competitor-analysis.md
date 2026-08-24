# csv-ragged-row-padder — competitor analysis (2026-08-20)

Scan run BEFORE implementing. One web search ("fix CSV ragged rows inconsistent
number of fields pad short rows online tool"); top reachable competitors skimmed
and paraphrased below. **No competitor copy, wording or branding was reused** —
only the feature set was compared.

## Competitors skimmed

| # | Tool | Reachable | Notes |
|---|------|-----------|-------|
| 1 | csvkit.org "Repair a Broken CSV" | yes | Richest option set of the three; explicit ragged-row strategy selector. |
| 2 | jsonparser.ai "CSV Fixer" | yes | Zero-config: auto-detect + pad/trim to header width. Documents a hard 64 KB input cap. |
| 3 | csvjson.tools "Fix Broken CSV Files" (guide + linked cleaner) | yes | Diagnostics-first: report ragged rows with line number + actual width before fixing. |
| — | encode64.com CSV Repairer | **no** (HTTP 403) | Could not be read; excluded from the table-stakes list. |

## Table stakes observed

| Capability | Seen at | Default they use | In model? | Decision |
|---|---|---|---|---|
| Pad short rows with empty fields | 1, 2 | on | in | `pad_value` (default empty string) — always pads |
| Truncate over-long rows | 1, 2 | truncate | in | `long_rows = truncate` (default) |
| Merge extra fields into the last column | 1 | — | in | `long_rows = merge` |
| Quarantine / list damaged rows instead of editing | 1, 3 | — | in | `long_rows = flag` (keep row, list it) and `long_rows = drop` |
| Target width taken from the header row | 1, 2 | header | in | `width_from = header` (default) |
| Width from widest / most-common row | implied by 1 ("actual widths") | — | in | `width_from = max` / `mode` |
| Explicit fixed width | — (gap vs all three) | — | in | `width` integer, `0` = infer |
| Delimiter auto-detection (`,` `;` tab `\|`) | 1, 2 | auto | in | `delimiter = auto` (default), or a name/char |
| Drop fully blank rows | 1 | on | in | `drop_empty_rows` (default true) |
| Strip UTF-8 BOM | 1 | on | in | always on, no param (documented) |
| Normalize line endings | 1 (→ LF) | LF | in | `line_ending = lf` (default) / `crlf` |
| Proper re-quoting of output | 1, 2 | on | in | always on (the `csv` writer quotes when needed) |
| Diagnostic report: counts + affected line numbers + actual widths | 1, 3 | — | in | `output = report` |
| Runs locally, no upload | 1, 2, 3 | — | in | already true — wasm in the browser |
| Whitespace trimming around fields | 1 (optional) | off | in | **out of scope**: a dedicated sibling tool already does exactly this; adding it here would duplicate it |
| Unclosed-quote repair / reconstruction | 1, 2 | — | **out of model** for this tool | Guessing where a runaway quote should have closed is a different (lossy, heuristic) job than width normalization; the parser reports the parse error instead. Listed, not built. |
| File upload + streaming for GB-scale files | 2 mentions punting to CLI | — | partly out | The page is a paste box (in-browser, no upload); the `gizza` CLI covers scripted/large use. Documented as a limit. |
| Stated input size cap | 2 (64 KB) | — | in | 5 MB cap — far above theirs — with a clear error message |

## UX patterns worth copying (patterns, not copy)

- **Strategy selector for long rows** rather than a hard-coded behavior (csvkit) —
  mirrored as the `long_rows` enum with four choices.
- **Report mode that names line numbers and their real widths** (csvjson's
  diagnostics-first framing) — mirrored as `output = report`, which lists every
  padded/truncated/merged/dropped/flagged row as `line N: X fields -> …`.
- **Zero-config happy path** (jsonparser) — every param except `data` has a
  default, so pasting a CSV and pressing run already does the right thing.
- **Preset chips** — none of the three ship presets; added anyway (`[[example]]`)
  for the two most common repairs (pad to header width; merge overflow).

## Gaps closed in this build

All in-model rows above are implemented. The two deliberate non-goals are
whitespace trimming (duplicates an existing sibling tool) and unclosed-quote
reconstruction (out of model — heuristic and lossy).

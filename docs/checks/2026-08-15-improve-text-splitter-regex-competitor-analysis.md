# text-splitter-regex — competitor analysis (2026-08-15)

Scan run **before** implementation, per the create-next-tool recipe. All findings are paraphrased
observations of publicly documented feature sets; no competitor copy, branding, or trademarks are
reproduced or reused anywhere in this tool.

## Tools reviewed

| # | Tool | URL | Notes |
|---|------|-----|-------|
| 1 | WebToolsOnline — Text Splitter | https://www.webtoolsonline.org/tools/text-splitter | Six split modes incl. regex; trim / remove-empty; four output formats; live stats |
| 2 | Online Text Tools — Split Text | https://onlinetexttools.com/split-text | Symbol / regex / length / chunk-count modes; configurable output separator + per-chunk prefix/suffix; five worked examples |
| 3 | ToolsForTexts — Text Splitter | https://www.toolsfortexts.com/text-splitter | Delimiter / character / word / line / regex modes; explicit regex-flag control; delimiter presets; output separator presets; per-chunk cards |
| 4 | Online String Tools — Split a String | https://onlinestringtools.com/split-string | Split by characters, regex, length, or substring count |
| 5 | Browserling — Split Text | https://www.browserling.com/tools/text-split | Minimal splitter; regex + literal separators |

Common ground: all five are browser-local, free, and treat "regex mode" as the power-user escape
hatch for mixed/repeated separators. None of them documents an input size limit.

## Table stakes → decision

| Capability | Seen at | Verdict | Where it landed |
|---|---|---|---|
| Split on a regular expression | 1,2,3,4,5 | **in-model** | `pattern` (required) |
| Multi-character / repeated-whitespace separators (`\s+`, `[,;\|]`) | 1,2,3 | **in-model** | native regex; documented + preset chips |
| Regex flag control (i, m, s) | 3 | **in-model** | `ignore_case`, `multiline`, `dotall` booleans |
| Trim each part | 1,3 | **in-model** | `trim` |
| Remove empty parts | 1,3 | **in-model** | `remove_empty` |
| Output format: one-per-line | 1,2,3 | **in-model** | `output = "lines"` (default) |
| Output format: JSON array | 1 | **in-model** | `output = "json"` |
| Output format: CSV row | 1 | **in-model** | `output = "csv"` (+ `"tsv"`) |
| Numbered parts | 3 (numbered cards) | **in-model** | `output = "numbered"` |
| Custom join separator (incl. blank-line / `---` / `, ` presets) | 2,3 | **in-model** | `output = "separator"` + `separator` (escapes `\n`, `\t`, `\r`, `\\`) |
| Limit the number of splits | (implicit in 2's chunk-count) | **in-model** | `max_splits` (0 = unlimited; remainder kept intact) |
| Split into **fields as well as rows** (text → columns) | 1 ("CSV row"), the Data School / ListShift text-to-columns framing | **in-model** | `field_pattern` — second-level regex giving a real 2-D table |
| Delimiter quick presets | 3 | **in-model** | `[[example]]` preset chips on the page |
| Escaped delimiters (`\n`, `\t`) | 1,3 | **in-model** | native in a regex pattern; `separator` unescapes them too |
| Per-chunk prefix/suffix strings | 2 | **out-of-model** | cosmetic wrapping; `separator` + JSON/CSV cover the real uses |
| Live stats (count / longest / average part) | 1,3 | **out-of-model** | page-runtime UI concern, not a text transform |
| Per-chunk copy buttons, chunk cards | 3 | **out-of-model** | generator renders one output pane (with a Download link for `format = "text"`) |
| Fixed-length / N-words / N-lines chunking | 1,2,3,4 | **covered elsewhere** | already shipped as `blocks/chunk-text` (size/overlap/unit/boundary) — not duplicated here |
| Literal-substring splitting | 1,2,3,4,5 | **covered elsewhere** | already shipped as `blocks/split-text` (literal / whitespace / chars) |

Nothing from the scan was dropped silently: every row above is either a descriptor param, an
explicit out-of-model entry, or an existing sibling block.

## Duplicate check (why this is a distinct tool)

- `blocks/split-text` — literal substring only ("set delimiter to any substring"); no regex.
- `blocks/regex-extract` — returns the *matches* of a pattern, the inverse of splitting *on* it.
- `blocks/regex-capture-to-csv` — one row per match with capture groups as columns; requires the
  data to be matched, not delimited.
- `blocks/chunk-text` — size-based chunking for RAG, not delimiter-based.
- `blocks/text-to-table` — renders already-delimited text as an aligned table.

Regex-delimiter splitting (and the row + field two-level split) is not offered by any of them.

## Gaps we close that the competitors do not

- **Two-level split.** Competitors split one dimension only; `field_pattern` turns delimited text
  into a real table (`output = "csv"` / `"tsv"` / `"json"` gives nested rows), which is the actual
  job when someone pastes log lines or a fixed-width dump.
- **Documented limits.** None of the five states an input cap; ours states 200,000 characters and
  100,000 parts on the page and returns an actionable error instead of freezing the tab.
- **Same behaviour on three surfaces.** The identical Rust core backs the page, the `gizza` CLI,
  and the chat block — competitors are page-only.

## Rules honoured

No competitor copy, wording, branding, or trademarks were copied. Out-of-model items are listed
here, not built. Page copy stays generic/brand-free per the hygiene gate.

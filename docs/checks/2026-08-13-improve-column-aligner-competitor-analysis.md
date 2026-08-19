# column-aligner — competitor analysis (2026-08-13)

Scan run BEFORE implementation, per `/improve-tool` Phase 2–3. All findings are **paraphrased**;
no competitor copy, branding, or trademarks are reproduced here or in the tool.

## Landscape

"Align text into columns" is a crowded but shallow category: most entrants are a single textarea
plus one button, and the reference implementation everyone is imitating is the Unix `column -t`
command (and `awk`/`printf` recipes for it).

| # | Competitor | Kind | What it is |
|---|-----------|------|------------|
| 1 | Browserling "Format Text Columns" | Web tool | Paste → one "align columns" button. Left-align only; the page itself notes other alignments are not implemented yet. Companion left-pad/right-pad/center-string tools live next to it. |
| 2 | Online Text Tools "Convert Text to Nice Columns" | Web tool | The most configurable of the group: separate **input** element/line separators and **output** column/row separators, left/right/center, "align separators", "separate empty elements". Free tier is personal-use with a daily cap. |
| 3 | DevOven "Text Alignment Tool" | Web tool | Five modes in one control: left / right / center / "column align" (tab-delimited only) / word-wrap at a width. Live char + line counts. |
| 4 | EasyPro Tools "Column Formatter" | Web tool | Auto-alignment of pasted list/CSV data into columns; minimal options. |
| 5 | ListDiff "Text Columnizer" | Web tool | Delimiter picker (tab/CSV/…) plus a justification choice. |
| — | `column -t` / `awk` recipes | CLI | The behavioral reference: split on whitespace runs, pad every column to its widest cell, no borders. |

Adjacent but **not** competitors: bordered-table renderers (ASCII box grids, Markdown pipe tables).
This repo already ships those as `text-to-table`, `csv-to-table` and `markdown-table-format`. The
distinguishing property of this tool is that the output stays **plain text with no borders and no
header row** — the input, only aligned — which is what `column -t` users actually want.

## Table-stakes parameters (and where we landed)

| Table stake | Seen in | Our decision |
|---|---|---|
| Choose the **input field delimiter** | 2, 3, 5 | `delimiter`, default `whitespace` (runs of spaces/tabs, the `column -t` behavior); also accepts `tab`, `comma`, `semicolon`, `pipe`, `space`, or any literal string (`" - "`, `"::"`) |
| **Left / right / center** alignment | 2, 3, 5 | `align`, default `left` |
| **Output column separator** character | 2 | `separator`, default empty (spaces only); any string, drawn centered in the gap (`name | value`) |
| Auto-align tab-delimited data with no config | 3, 4 | Default config already does it — whitespace runs cover tabs |
| Spacing control between columns | — (implicit) | `gap`, default `2` spaces (matches `column -t`), 0–16 |
| Trim padding around each field | 2 (implicit in "elements") | `trim`, default on |
| Fixed-width padding of ragged rows | 1, 2, 3 | Short rows are padded to the column count; **no trailing whitespace is ever emitted** |

## Gaps we close that no scanned competitor does

- **`align = auto`** — columns whose non-empty cells are all numeric are right-aligned, everything
  else left-aligned. This is the layout people actually hand-build for reports; none of the scanned
  tools detect numeric columns.
- **Per-column alignment** (`column_align`, e.g. `lrr` or `l,r,c`) — competitors apply one alignment
  to the whole document. Shorter specs fall back to `align` for the remaining columns.
- **East-Asian / emoji width correctness** — padding is computed with Unicode display width, so CJK
  text lines up in a monospace font. Naive char-count padding (what a JS `padEnd` does) visibly
  skews these columns.
- **Stated limits on the page** — 20,000 lines and 512 columns, with errors that name the limit and
  the value that exceeded it.

## Considered, not built

- **Word-wrap mode** (3) — that is a different tool; this repo already ships `text-wrap`.
- **Output row separator / "align separators" / "separate empty elements"** (2) — niche options that
  each add a parameter for a layout the `separator` + `trim` pair already approximates.
- **Bordered ASCII / Markdown table output** (adjacent tools) — deliberately out of scope; that is
  `text-to-table`'s job, and duplicating it here would blur both tools.
- **File import / QR share / social share buttons** (2, 3) — platform chrome, not tool capability.
- **Accounts, daily caps, paid tiers** (2) — out of model; this tool runs fully in the browser with
  no account and no upload.

## UX controls adopted

- `<select>` for `align` (fixed vocabulary → `Param::enumv`); checkbox for `trim`; slider for `gap`.
- `multiline` textarea for the input so pasted line breaks survive.
- Placeholders on every text/number field showing a real value (`whitespace`, `lrr`, `|`, `2`).
- Example chips for the presets these tools ship as separate buttons: whitespace `column -t`,
  CSV, numeric right-align, pipe separator, and per-column alignment.
- Worked input→output pair, an explicit limits section, and FAQ accordions covering delimiter
  choice, trailing whitespace, Unicode width, and why the output has no borders.

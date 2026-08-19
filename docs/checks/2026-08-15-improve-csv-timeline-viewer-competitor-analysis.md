# csv-timeline-viewer — competitor analysis (2026-08-15)

Scan run BEFORE implementation, per `/create-next-tool` step 3. One web search
(`online CSV log timeline viewer filter search time range browser tool`), then the top
reachable competitors were inspected directly. All notes below are **paraphrased**; no
competitor copy, branding, or trademark is reused anywhere in this tool.

## Competitors inspected

| # | Competitor | Reachable | What it is |
|---|---|---|---|
| 1 | Jam log/CSV file viewer (`jam.dev/utilities/csv-file-viewer`) | yes | Browser-local viewer for `.csv/.tsv/.txt/.log` with faceted column filtering and row tracing |
| 2 | ToolsRail log file analyzer (`toolsrail.com/file-tools/log-file-analyzer.php`) | yes | Browser-local log analyzer with level filters, regex search, time-range presets and an activity graph |
| 3 | DevToolLab CSV viewer (`devtoollab.com/tools/csv-viewer`) | yes | Browser-local CSV/TSV table viewer with sortable columns and a cross-column search box |
| — | SANS Timeline Explorer (`sans.org/tools/timeline-explorer`) | reachable but thin | Desktop DFIR CSV/Excel timeline grid (filter/group/sort); listed as the domain reference, not a web competitor |
| — | DvTools advanced log viewer (`dvtools.in/tools/log-viewer`) | **no — HTTP 403** | Replaced by DevToolLab (#3) per the "replace unreachable" rule |

## Table stakes observed (all three web competitors)

- Runs entirely client-side; input never leaves the machine — stated prominently.
- One search box that matches across **every** column, not a single named field.
- Sort by any column, both directions.
- Multiple delimiters accepted (comma, semicolon, tab, pipe) with auto-detection.
- Row numbering so a row can be referred back to its position in the source file.
- Header-row toggle for headerless data.
- Stated size guidance rather than a silent failure on big files.

## Differentiators worth having

- **Time-range filtering** (ToolsRail): `from`/`to` bounds over a detected timestamp column.
  This is the row's actual premise and no CSV-viewer competitor does it well.
- **Regex search** (ToolsRail): opt-in, alongside the default plain-substring search.
- **Column-condition filters** (Jam's facet dropdowns, Timeline Explorer's per-column filters):
  our declarative equivalent is a multi-line `filters` field (`col op value`, AND-ed).
- **Activity histogram** (ToolsRail's spike graph): our `output = summary` mode buckets
  matches over time and prints counts + bars, which is the same insight in text form.
- **Column projection** (Timeline Explorer's column chooser): a `columns` field.
- **JSONL input** — none of the three take JSON Lines; the backlog row explicitly asks for it.

## Defaults chosen (and why)

| Setting | Default | Rationale |
|---|---|---|
| `format` / `delimiter` | `auto` | Every competitor auto-detects; making the user pick first is a worse first run. |
| `header` | `true` | Overwhelmingly the common case for event exports. |
| `time_column` | auto-detect | Competitors that do time ranges detect the column; asking for it up front is friction. |
| `order` | `asc` | A timeline reads oldest-first by default; `desc` is one click for "what happened last". |
| `limit` / `offset` | `100` / `0` | A viewer shows a page, not the whole file; paging matches the competitors' virtualized grids. |
| `output` | `table` | The competitors' primary surface is a readable aligned grid, so ours is too. |
| `regex`, `case_sensitive` | `false` | Plain case-insensitive substring is what a search box does by default. |

## UX decisions taken from the scan

- Aligned text table carries a leading `#` column with the **source row number**, so a row
  found after sorting/filtering can still be located in the original file (Jam's row-trace idea,
  implemented declaratively).
- A footer line always states `showing X-Y of N matched (M rows read)` — the competitors all show
  a match count; silence about how much was filtered away is the main way a viewer misleads.
- `[[example]]` preset chips replace the competitors' "Find all errors / Last hour / Today" quick
  actions, since a chip is this platform's declarative preset control.
- Limits (200k input lines, 60-char cell truncation in table mode, 100k row cap) are stated on the
  page, not discovered through an error.
- Errors name the offending column/line and list what was available, rather than a bare "invalid".

## Considered, not built (out of model)

- **File upload / drag-and-drop of a 50 MB file** — the page takes pasted text; the platform's
  file-input path is for media (ffmpeg) tools. Paste + the CLI cover the same data.
- **Relative ranges ("Last hour", "Today")** — these need a wall clock; the page's
  wasm32-unknown-unknown target has no std clock and the tool's output must stay deterministic
  for a given input. Explicit `from`/`to` values (and example chips) cover the same need.
- **Rendered activity charts / colored severity rows** — the page output surface is text; the
  `summary` histogram is the in-model equivalent. Charting a CSV is already `csv-chart-generator`.
- **Live tail / auto-scroll** — no streaming input in this model.
- **Interactive click-to-facet and multi-row trace marking** — needs stateful per-tool JS; the
  shared generator is declarative by design, and adding a slug-specific runtime branch is
  explicitly banned by the platform rules.

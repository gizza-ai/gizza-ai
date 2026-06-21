# markdown-table-format — competitor analysis (2026-06-21)

Tool: **Markdown Table Formatter** — aligns and pretty-prints GitHub-flavored
Markdown pipe tables. Surfaces verified: chat block (`wafer build` OK), CLI
(`gizza tool markdown-table-format`), and the standalone page (Playwright, 5
specs green).

## Competitors surveyed

1. **markdowntable.com** — pads cells to line up pipes in a monospace font; basic,
   no alignment control or modes.
2. **CodeShack Markdown Table Generator** — visual grid editor; Pretty Print vs
   **Compact Mode**; alignment buttons emit `:---` / `:---:` / `---:`; paste from
   Excel/Sheets.
3. **Table to Markdown — Format Markdown Table** — one-click prettify with
   customization; grid/visual oriented.
4. **Markdown Table Prettify (VS Code, darkriszty)** — alignment via `:`,
   configurable column padding, **compact mode** (drops trailing border when no
   leading border), find/format **multiple tables**, respects code blocks,
   indented tables. **Known limitation: CJK / mixed-width chars "not always
   properly formatted."**
5. **TableConvert / TablesGenerator** — full visual editors: transpose, sort,
   dedupe, regex find/replace, import from spreadsheets/HTML; conversion-focused
   rather than pure reformatting.

## Gap analysis (fit-to-model)

Our model: a pure text-in / text-out reformatter with named params, no visual
grid editor (out of scope for the chat/CLI/page surfaces).

| Capability | Competitors | Us (before) | Action |
|---|---|---|---|
| Even-width padding / aligned grid | all | yes | kept (pretty) |
| Preserve `:---`/`---:`/`:---:` alignment | most | yes | kept (`align=keep`) |
| Force a single alignment for all columns | CodeShack (buttons) | yes | kept (`align=left/center/right`) |
| **Compact / minified layout** | CodeShack, VS Code | **no** | **ADDED `style=compact`** |
| **Respect fenced code blocks** | VS Code | **no** | **ADDED ``` / ~~~ tracking** |
| Multiple tables in one document | VS Code | yes | already supported |
| **CJK / wide-char width** | none (VS Code can't) | **yes** | **differentiator** |
| Ragged-row padding to full column count | most | yes | already supported |
| Escaped `\|` kept inside a cell | varies | yes | already supported |

### Closed this pass (in-model)

- **`style=compact`** — single-space padding, no width alignment, for the
  smallest diff-friendly output (matches CodeShack's "Compact Mode" / VS Code's
  compact). `style=pretty` (default) keeps the aligned grid.
- **Fenced-code-block awareness** — a `|---|` line inside ` ``` ` or `~~~` is left
  byte-for-byte, so example tables in docs aren't clobbered (matches VS Code).

### Already ahead

- **Unicode wide-character widths** (CJK, fullwidth, emoji) — columns line up even
  in mixed-script tables. The leading VS Code prettifier explicitly does **not**
  handle this.

## Out of model (not built — by design)

- **Visual grid editor / spreadsheet paste** (TablesGenerator, TableConvert,
  CodeShack) — requires an interactive table UI, outside the text-in/text-out
  contract of all three gizza surfaces.
- **Data operations** (transpose, sort, dedupe, regex find/replace — TableConvert)
  — these are separate transforms; gizza already ships dedicated CSV tools
  (csv-transpose, csv-dedupe, csv-query, find-replace) for that work.
- **HTML / CSV import** — covered by the existing `csv-to-table` and
  `html-table-extractor` tools.

No competitor copy, branding, or trademarks were used.

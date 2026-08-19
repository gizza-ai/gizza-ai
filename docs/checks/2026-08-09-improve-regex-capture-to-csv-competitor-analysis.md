# regex-capture-to-csv — competitor analysis (2026-08-09)

Scan run **before** implementing, per the create-next-tool recipe. All findings are paraphrased
observations of publicly documented behaviour; no competitor copy, branding, or trademarks are
reproduced or reused, and none of their wording appears in our page.

## Scope check (why this is not a duplicate)

`blocks/regex-to-json` is the nearest sibling and was inspected before building:

- it is **line-oriented** — it splits the input into lines and matches within each line, so a match
  can never span lines;
- its output shapes are JSON / compact JSON / NDJSON only — no CSV, no delimiter, no quoting, no
  column selection.

This tool scans the **whole text** (so `dotall` matches can span lines) and emits RFC-4180 CSV with
delimiter, quoting, header, column selection/reordering, dedupe and sort. Different extraction
semantics plus a different output contract → built, not skiplisted.

## Competitors reviewed

1. **ConvertCSV "Regex Text Extractor"** (convertcsv.com/text-extractor.htm) — the closest match:
   regex over pasted text/file/URL, results written out as CSV.
2. **CSV Tools Online "Regex Tester & Extractor"** (csvtoolsonline.com/tools/regex-tester) — regex
   run against a chosen column of an uploaded CSV/XLSX, results exportable as CSV.
3. **Regex101** (regex101.com) — the reference regex workbench: flag toggles, named-group
   inspection, "export matches".

## Table stakes → decision

| Capability (observed) | Seen at | In model? | Where it landed |
| --- | --- | --- | --- |
| Regex pattern over pasted text | all three | in-model | `pattern` (required) |
| Named capture groups → named columns | 2, 3 | in-model | header from `(?<name>…)`/`(?P<name>…)`; unnamed → `column1…`; no groups → `match` |
| Case-insensitive / multiline / dotall flags | 2, 3 | in-model | `ignore_case`, `multiline`, `dotall` |
| Output delimiter (comma, semicolon, tab, pipe, custom) | 1 | in-model | `delimiter` (single char, `\t`, or keyword) |
| Header row toggle | 1 | in-model | `header` (default true) |
| "Force CSV-compliant" quoting | 1 | in-model | `quoting` = `minimal` \| `all`, RFC-4180 quote doubling |
| EOL format (CRLF vs LF) | 1 | in-model | `line_ending` = `lf` \| `crlf` |
| Deduplicate results | 1 | in-model | `unique` |
| Sort results | 1 | in-model | `sort` |
| Choose which columns / how many per row | 1, 2 | in-model | `columns` — subset **and** reorder, unknown name errors with the available list |
| Preset pattern buttons (numbers, words, dates, HTML tags) | 1 | in-model (as page chips) | three `[[example]]` preset chips: access log → CSV, key=value deduped+sorted, HTML cells → TSV |
| Download result as a file | 1 | in-model (platform) | automatic: `format = "text"` pages get a Download link + Copy/Reset from the generator |
| Regex flavour with lookaround/backreferences | 3 (PCRE2) | **out-of-model** | Rust `regex` is linear-time and has neither; stated on the page and in the "no matches" FAQ |
| File upload / fetch a URL / batch-scan many pages | 1, 2 | **out-of-model** | pure text-in/text-out block; paste (or pipe via the CLI) instead |
| Input encodings beyond UTF-8 (ISO-8859, Windows codepages) | 1 | **out-of-model** | inputs are UTF-8 strings across chat/CLI/page |
| Read a column out of an uploaded XLSX/CSV | 2 | **out-of-model** here | already covered by existing blocks (`xlsx-to-csv`, `csv-query`) — chain them |
| Min/max length filters, lowercase transform, "contains" post-filter | 1 | **out-of-model** here | post-processing belongs to the existing CSV blocks (`csv-filter`, `csv-sort`); not duplicated |
| Live match highlighting / explanation pane | 3 | **out-of-model** | `regex-tester` already covers interactive debugging |

Nothing observed was dropped silently: every row above is either implemented or listed as
out-of-model with the reason.

## UX patterns adopted

- Preset chips instead of a bespoke "quick pattern" button row (the generator's declarative
  `[[example]]` mechanism).
- Friendly `<select>` labels via `[input.labels]` for `quoting` and `line_ending` so the canonical
  values stay machine-clean (`minimal`/`all`, `lf`/`crlf`).
- `multiline = true` on the text field so pasted multi-line input keeps its newlines.
- Placeholders on every text field, worked example + stated limits in the page copy.

## Deliberate behavioural choices

- **Zero matches is an error**, not an empty file — on an interactive page a silent empty result
  reads as a broken tool, and it is nearly always a pattern bug. Documented in the FAQ.
- **Caps:** 1 MB input and 100,000 rows, both with actionable errors (a pattern that can match the
  empty string otherwise produces a row per byte).
- **Optional groups yield empty fields** so every row has identical arity — required for a CSV that
  a spreadsheet can open.

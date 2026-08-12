# csv-row-index-adder — competitor analysis (2026-08-12)

Scan run **before** implementation. One WebSearch ("add row number index column to CSV online
tool UUID column generator"), then three real competitor surfaces were read. Everything below is
paraphrased observation of behaviour and option sets — no competitor copy, naming, or branding is
reproduced or reused.

## Surfaces reviewed

| # | Surface | What it is | Why it counts |
|---|---------|-----------|---------------|
| 1 | csvtools.com — "Add Row Numbers" | Browser-local CSV tool, the closest single-purpose analogue | Same job, same "paste and go" model |
| 2 | Easy Data Transform — UUID transform (docs) | Desktop data-prep app; adds a generated-UUID column to a table | The only mainstream "UUID column onto an existing table" flow |
| 3 | Microsoft Power Query — Add Index Column (docs) | The de-facto reference semantics for index columns | Defines what users expect "start"/"increment" to mean |

Also noted from the search page (not counted as one of the three, no distinct table-stakes):
bulk UUID generators (generateuuids.com, csvtoolsonline GUID generator) — these *emit* a list of
UUIDs, they do not attach one per row of an existing table; that generate-a-list job is already
covered by this repo's `generate-uuid` block.

## Table-stakes extracted, and where each landed

| Table-stake | Seen on | Decision | Where |
|---|---|---|---|
| Paste a CSV/TSV as text | 1 | **In model** | `data` (multiline textarea) |
| Header-row present toggle | 1 | **In model** | `has_header` (default on) |
| Custom column heading | 1, 2 | **In model** | `column_name` (blank = per-mode default) |
| Custom start number | 1, 3 | **In model** | `start` (default 1; 0 gives Power Query's zero-based default) |
| Custom increment / step | 3 | **In model** | `step` (default 1; negatives allowed, 0 rejected) |
| Column placed first (leftmost) | 1 | **In model** | `position` default `start` — matches competitor 1's fixed behaviour, but ours is configurable |
| Column placed elsewhere | 3 (Power Query appends; 1 is leftmost-only) | **In model, exceeds both** | `position` = `start`/`end`/`before`/`after` + `reference_column` |
| Generated UUID column | 2 | **In model** | `mode = uuid`, using RFC 4122/9562-compatible v4/v7 formatting in this block |
| UUID format variants (hyphens/braces) | 2 | **In model** | `uuid_format` = `standard`/`uppercase`/`compact`/`braces`/`urn` |
| Delimiter choice / auto-detect | 1 | **In model** | `delimiter` = `auto`/`,`/`tab`/`;`/`\|` |
| Copy + download the result | 1 | **In model, already generic** | the generator gives `format = "text"` pages a copy control and a Download link — nothing to build |
| Zero-padded ids (`001`) | none of the three | **In model, differentiator** | `pad_width` |
| Prefixed/suffixed ids (`INV-0001`) | none of the three | **In model, differentiator** | `prefix`, `suffix` |
| Composite/business key from existing columns | none of the three | **In model, differentiator** | `mode = composite`, `columns`, `separator` |
| Time-ordered (sortable) ids | none of the three | **In model, differentiator** | `uuid_version = 7` |
| File-drop and fetch-by-URL input | 1 | **Out of model** | the pure-tool page surface takes pasted text; file/URL sources exist only on the media (ffmpeg) runtime here |
| Quote character + comment-symbol parse options | 1 | **Out of model (deliberate)** | RFC 4180 double-quote parsing only; a custom quote char is a parser-config knob no other CSV block here exposes, and adding it to one block alone would be inconsistent |
| "Skip empty lines" toggle | 1 | **Out of model (already the behaviour)** | the `csv` reader drops fully blank lines; a toggle to *keep* them would mean emitting index values for rows that do not exist |
| Auto-run / live re-generation on every edit | 2 | **Out of model** | desktop-app pipeline feature; the page already re-runs on input change |
| Millions-of-rows datasets | 2 | **Out of model** | capped at 5,000,000 bytes so a runaway paste cannot wedge the tab (stated on the page) |
| Random *sample-data* generation (whole synthetic tables) | search results | **Out of scope** | a different tool (generate a table), not adding a key to one you have |

## UX patterns adopted

- Competitor 1 hides everything behind an options panel and exposes only heading + start number.
  Ours shows every control inline, and the two that matter most (mode, position) are `<select>`s so
  the choice set is visible rather than discovered.
- Competitor 1 ships one sample-data button. Ours ships five `[[example]]` preset chips, one per
  real use case (sequential id, zero-padded invoice number, UUID v4 key, sortable UUID v7,
  composite key), which is the same "click to see it work" affordance at higher coverage.
- Competitor 3's dialog is start + increment only, presented as "Custom" vs a zero-based default.
  We keep the same two knobs with the same meanings so the semantics transfer, and default to
  `start = 1` (spreadsheet-style) with `start = 0` documented for the Power Query convention.

## Overlap check against existing blocks (dup review)

- `csv-insert-column` inserts a **constant** value into every row — it cannot vary per row, so it
  covers none of sequential/UUID/composite.
- `csv-window-functions` has a `row_number` window function, but it requires a header row, always
  appends at the end, and offers no start/step/padding/prefix/UUID/composite. Its framing is SQL
  window analytics (partition/order), not key generation.
- `generate-uuid` emits a standalone list of UUIDs; it never sees your table.

Distinct capability confirmed: per-row generated key columns (sequential, UUID, composite) with
placement, padding, and affix control. Built rather than skiplisted.

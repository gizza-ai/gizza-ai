# csv-whitespace-normalizer — competitor analysis (2026-08-17)

Scan run **before** implementation, so the descriptor shipped with the table stakes already in
it. All findings below are **paraphrased** from public tool pages — no competitor copy, branding,
or trademark is reproduced anywhere in this repo.

## Duplicate check (run first)

| existing block | overlap | verdict |
| --- | --- | --- |
| `blocks/csv-cleaner` | has a `trim` toggle: `f.trim()` per cell, i.e. leading/trailing only | **not a duplicate** — it cannot collapse *internal* runs of whitespace, which is this tool's namesake capability. It is a multi-op cleaner (dedupe/empty rows/fill/delimiter) where trim is one switch. |
| `blocks/remove-whitespace` | collapses whitespace in **free text**, line-oriented | not a duplicate — CSV-unaware; it would rewrite delimiters, quoting and record structure. |
| `blocks/text-normalizer` | Unicode NFKC / case / accents / whitespace on free text | not a duplicate — same CSV-unaware problem. |
| `blocks/tabs-to-spaces`, `blocks/zero-width-cleaner`, `blocks/smart-quotes-clean` | single-character-class text fixes | not duplicates. |
| `blocks/csv-header-sanitizer`, `blocks/csv-null-standardizer` | per-cell CSV rewrites of a *different* token class | not duplicates; same family, disjoint jobs. |

No skiplist line was added. The distinct capability is per-cell **internal** whitespace
collapsing/removal inside a real RFC 4180 parse, with column scoping.

## Competitor profiles (top 5 reachable tools + 1 sibling page)

Two candidates from the search results were dropped for a 403 login/bot wall
(`cleanchart.app/tools/whitespace-cleaner`, `beantoolbox.com/tools/csv-normalize-tool`) and
replaced, per the skill's "replace, don't run with 4" rule.

### 1. onlinetools.com — "trim csv columns"
- **Features (paraphrased):** strips leading/trailing whitespace from column values; client-side.
- **Params/options:** column selection (all by default; 1-based positions, ranges like `2-5`,
  lists, negative index for the last column, or header names one per line); left-trim /
  right-trim / both; a separate "trim headers" switch; drop comment lines by symbol; drop empty
  lines; dedupe-header option.
- **I/O:** paste or upload; copy, download, or export to a paste service.
- **Limits:** free tier is personal-use with daily caps; commercial/unlimited use is paid.
- **UX:** side-by-side input/output editor, three preset example configs, URL parameters,
  chaining to sibling CSV tools.

### 2. csvtools.com — "trim whitespace"
- **Features:** removes leading/trailing spaces from every cell, explicitly *preserving* spaces
  inside a value; in-browser.
- **Params/options:** column separator (auto-detect by default, or custom), quote character,
  comment symbol (off by default), header-row present toggle, skip-empty-lines toggle.
- **I/O:** paste / file drop / fetch by URL → copy or download `.csv`.
- **UX:** drag-and-drop zone, "load sample data" link, an empty output pane with placeholder
  text, an expandable advanced-options section, 3 FAQ entries (privacy, interior spaces, headers).

### 3. csvtools.com — "remove all whitespace" (sibling page of #2)
- **Features:** the interior half — collapses runs of consecutive spaces, tabs and other
  whitespace characters inside a cell down to a single space, *and* trims the ends. States that
  tabs and newlines inside cells are normalized to spaces.
- **Params/options:** same option set as #2 (separator, quote char, comment symbol, header,
  skip empty lines).
- **Notable:** their own FAQ frames the split as two separate tools — trim-only vs
  trim + collapse. That split is the single most useful design signal from the whole scan.

### 4. happycsv.com — "trim whitespace"
- **Features:** trims leading/trailing whitespace from every cell, keeps interior spacing,
  processes in a background worker for large files.
- **Params/options:** none exposed — a fixed, zero-configuration pass.
- **I/O:** file upload (or sample data) → download.
- **UX:** three-step upload → process → download flow; FAQ covers scope, why stray spaces break
  joins, tabs/newlines, and privacy.

### 5. onlineminitools.com — "trim csv columns"
- **Features:** trim plus an explicit "clean internal spaces" toggle; column targeting.
- **Params/options:** trim action (leading & trailing (default) / leading only / trailing only);
  collapse-internal-spaces toggle; columns (all by default, or picked as clickable tags);
  delimiter auto-detect with TSV support; "CSV has a header row" toggle.
- **I/O:** paste or upload → output box with copy/download.
- **SEO angles:** duplicate-key violations on import, `VLOOKUP`/`INDEX-MATCH`/SQL joins failing
  on invisible padding, smaller payloads.

### 6. hndytools.com — "csv whitespace trimmer"
- **Features:** trims cell values; column selection by number or header name.
- **Params/options:** trim all columns / by number / by header name; left-trim, right-trim (both
  on by default); trim-headers switch; remove comment lines; remove empty lines.
- **I/O:** CSV file in, cleaned CSV out; sample data available.
- **UX:** links onward to converter/viewer tools for multi-step workflows.

## Gap list → decisions (fit-to-model)

Model: browser-local, wasm, no account, no server, no upload.

**In-model, built into the first release:**

| gap | seen at | how it shipped |
| --- | --- | --- |
| trim direction is a choice, not a constant | #1, #5, #6 | `trim` = `both` \| `leading` \| `trailing` \| `none` (`Param::enumv`) |
| interior whitespace collapsing (the namesake) | #3, #5 | `internal` = `collapse` (default) \| `remove` \| `keep` — `remove` also covers the "strip every interior space" framing of #3's page title, useful for IDs/part numbers |
| column scoping instead of whole-file only | #1, #5, #6 | `columns` — comma-separated header names, 1-based positions, and `2-4` ranges; blank = every column |
| delimiter auto-detect + TSV/semicolon/pipe | #2, #3, #5 | `delimiter` = `auto` \| a single char \| `comma`/`tab`/`semicolon`/`pipe`; the input separator round-trips to the output |
| header row handled separately from data | #1, #2, #5, #6 | `header` (first row is a header) + `normalize_header` (apply the same pass to header cells; on by default, mirroring the competitors' "trim headers" switch) |
| tabs/newlines inside quoted cells | #3 (states it), #4 (FAQ) | any whitespace *run*, embedded newlines included, becomes one space under `collapse` — a real RFC 4180 parse, so embedded separators/newlines never break the record |
| worked examples / presets | #1, #2, #5 | five `[[example]]` chips + a worked input→output pair in the page copy |

**In-model but deliberately not built (reasons, not omissions):**

- *Remove comment lines / remove empty rows* (#1, #6) — already `blocks/csv-cleaner`'s job
  (`drop_empty_rows`), and duplicating it here would make two tools disagree over time.
- *Output quote-style control* — out of scope for a whitespace pass; minimal quoting is the safe
  default and `blocks/csv-null-standardizer` already exposes the knob for people who need it.
- *Clickable column tags* (#5) — the generator's `tag-list` control splits on commas, and CSV
  header names legitimately contain commas; a plain text selector stays honest here. (Same
  reasoning already recorded for other bulk-paste list fields.)

**Out-of-model (needs something gizza deliberately does not have):**

- File upload / drag-and-drop and fetch-by-URL input on the page (#1, #2, #3, #4, #6) — the
  generator's file input is the ffmpeg runtime's; pure tools take pasted text. The CLI covers the
  file case (`gizza tool csv-whitespace-normalizer data="$(cat file.csv)"`).
- Export to a third-party paste service (#1) — that is an upload, which contradicts the model.
- Accounts, daily quotas, and paid commercial tiers (#1) — no server, no accounts, no gating.
- Background-worker progress UI for very large files (#4) — the pass is synchronous wasm with a
  5,000,000-byte cap stated on the page.

**Our differentiator (no competitor in the scan states it):** a `whitespace` selector covering
the full Unicode `White_Space` set — non-breaking space (U+00A0), narrow NBSP, ideographic space
(U+3000) and friends — not just ASCII space/tab. Those are exactly the characters that survive a
copy-paste from a spreadsheet or a web page and then silently break a join, and every competitor
page in this scan talks about spaces and tabs only. `ascii` remains selectable for anyone who
must leave NBSP intact.

> Original work only — no competitor copy, branding, or trademarks were copied.

# date-column-validate — competitor analysis (2026-07-24)

Tool: **date-column-validate** — checks that every value in a chosen CSV date column
parses against a chosen date format (preset such as ISO `%Y-%m-%d`, or a custom chrono
strftime pattern, or RFC 3339). Browser-local, wasm, no account, no server.

## Top 3 competitors (paraphrased — no copy/branding reproduced)

### 1. ConvertCSV — CSV Validator (`convertcsv.com/csv-validator.html`)
- Per-column validation matrix; "Date" is one of several selectable column types.
- Flags every failing cell (per-cell granularity, red highlight, "show only invalid rows").
- Presets: ISO `YYYY-MM-DD`, `MM/DD/YYYY`, `DD/MM/YYYY`, plus Unix epoch s/ms.
- Delimiter auto-detect (comma/semicolon/tab/pipe/…), header override, required-field checks.
- Extras: time-series gap detection (missing dates at an interval); ZIP/phone/NPI/Luhn checks.
- Fully client-side, no account, no stated size cap.

### 2. BeCSV — CSV & Excel Data Validator (`becsv.com/csv-data-validator`)
- Column rules dropdown including "Date Format", plus Email/URL/Numeric-Range/Unique/Regex.
- Results dashboard: total rows, total issues, invalid rows, success rate (per-row level).
- Three-step Load → Set Rules → Verify/Export flow; exports cleaned data or a report.
- Runs locally; exact date-format tokens and delimiter/header handling not surfaced on page.

### 3. Selqio — Data Quality Checker (`selqio.com/tools/data-quality-checker`)
- Date + email + data-type validation, plus duplicate/outlier/missing-value detection.
- Per-column profiling and an overall quality % (completeness/uniqueness/validity/consistency).
- Assumes headers in row 1; date-format tokens not exposed; CSV/JSON in, JSON report out.
- Browser-based, no account; soft-recommends < 50k rows.

(ExtendsClass CSV Validator was evaluated and dropped — it checks structure/quoting/column
count only, with no date validation.)

## Gap decisions (fit-to-model)

### In-model — built into this tool
- **Explicit format token + presets.** A raw strftime pattern field (`%d-%b-%Y`, etc.) plus
  ISO / US / EU / ISO-datetime / RFC 3339 presets. None of the three cleanly expose a raw
  token field — this is the wedge. **Built** (`preset` enum + `format` string).
- **Per-cell error reporting.** Each offending cell lists row, line, value, and a reason that
  names the expected format. **Built** (`invalid_rows` with `message`).
- **Live summary counts.** Total checked, valid, invalid, truncation flag. **Built.**
- **Delimiter auto-detect + manual override** and **header on/off with column-by-name or
  0-based-index selection.** **Built** (`delimiter` enum, `has_header`, `column`).
- **Calendar-validity checks** (reject `2021-02-30`, month `13`, bad leap days). **Built for
  free** — chrono's `parse_from_str` rejects impossible calendar dates, not just bad shape.
- **Capped invalid list** so huge files stay readable; full invalid count still reported.
  **Built** (`max_issues`, `truncated`).
- **JSON + text output** for machine or human consumption. **Built** (`output` enum).

### In-model — considered, not built (kept the tool focused on single-column date validation)
- **Time-series gap detection** (missing dates at a daily/weekly/monthly interval). Distinct
  feature that belongs in its own gap-finder tool; would bloat a validator's schema/UX.
- **Format-ambiguity warnings** (`03/04/2021` = Mar 4 or Apr 3). The chosen preset/pattern
  already disambiguates by construction; an extra advisory layer adds UX noise for little gain.
- **Unix epoch (s/ms) as a "format".** Epoch is a numeric range check, not a date-string
  parse; `csv-column-type-validator`'s `int`/`float` types and range validators cover it better.

### Out-of-model (needs server/account/paid — listed, not built)
- Multi-column rule matrices, business-format libraries (NPI/SSN/phone/Luhn), and saved
  reusable rule profiles (persistence/accounts).
- Cross-file, scheduled, or API-driven validation.
- Very-large-dataset processing beyond browser memory (competitors themselves soft-cap client-side).
- Whole-file multi-dimensional "data quality score" dashboards (broader than this tool's lane).

### Relationship to the existing `csv-column-type-validator`
That tool validates many columns against coarse declared types (`int/float/bool/date/enum`)
with date limited to three fixed styles (`iso/us/eu/any`). **date-column-validate** is the
focused counterpart: one date column, an **arbitrary** strftime pattern or RFC 3339, and a
precise per-row invalid report — a capability the type validator does not offer. Not a duplicate.

> Original work only — no competitor copy, branding, or trademarks reproduced.

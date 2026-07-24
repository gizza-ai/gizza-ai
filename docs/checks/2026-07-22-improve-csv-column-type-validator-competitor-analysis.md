# Competitor analysis — csv-column-type-validator (2026-07-22)

Function: given a CSV and a per-column *declared* type schema (int / float / date /
bool / enum), check every cell against its column's type and list the offending
cells. All paraphrased — no competitor copy/branding reproduced.

## Competitors scanned

| # | tool | reachable | notes |
| - | ---- | --------- | ----- |
| 1 | ConvertCSV — CSV Validator (`convertcsv.com/csv-validator.html`) | yes | richest per-column rule matrix; browser-local |
| 2 | Teleport — CSV Validator (`goteleport.com/resources/tools/csv-validator/`) | yes (thin docs) | integer/string/date typing, per-column schema |
| 3 | Flipper File — CSV Validator (`flipperfile.com/text-tools/csv-validator/`) | yes | auto-*infers* types (int/number/email/date/text), in-browser, errors-only view |
| - | onlinetools.com / cleanmyexcel.io | listed | not deep-scanned; syntax-only or AI/cloud |

## Table-stakes (what ≥1 competitor ships)

- **Per-column type declaration** keyed by header name (ConvertCSV rule matrix; Teleport schema). ✅ our `types` field (`col:type`, comma/newline-separated).
- **Type set:** integer, number/float, boolean, date, enum/allowed-values, text. ✅ int/float/bool/date/enum (string always-valid → out of scope, documented).
- **Enum / allowed-values** per column (ConvertCSV "allowed values"; FeuTeX `enum(a|b|c)`). ✅ `enum(a|b|c)`.
- **Multiple date formats** (ConvertCSV: YYYY-MM-DD, MM/DD/YYYY, DD/MM/YYYY, DD-MON-YYYY, epoch, custom). ✅ `date_format` = iso / us / eu / any (covers the 3 common numeric layouts + accept-any).
- **Missing/empty-value handling** (ConvertCSV missing-value sentinels + Required; Flipper flags empty as error). ✅ `empty_ok` (empty cells pass by default; set false to require every checked cell non-empty). Sentinel list (NULL/NA/…) → considered, see below.
- **Delimiter handling / auto-detect.** ✅ `delimiter` auto/comma/tab/semicolon/pipe.
- **Offense report grouped by row + column with a summary count** (ConvertCSV live counts; Flipper row/col highlight + totals). ✅ report lists row, physical line, column, value, expected type + summary counts; `max_issues` cap.
- **Header vs headerless.** ✅ `header` bool; headerless → reference columns by 1-based index.

## Defaults chosen

- `header` = true, `delimiter` = auto, `date_format` = iso, `empty_ok` = true,
  `max_issues` = 50. Matches the common "first row is a header, ISO dates,
  blanks allowed" expectation.

## Worked example (table-stake: worked input→output)

types = `age:int, score:float, active:bool, joined:date, plan:enum(free|pro)`

```
name,age,score,active,joined,plan
Ada,34,9.5,true,2021-06-01,pro
Bo,twelve,8,yes,2021/06/02,gold
```

→ INVALID — 3 offending cell(s): row 2 age "twelve" not int; row 2 joined
"2021/06/02" not an iso date; row 2 plan "gold" not in enum(free|pro).

## In-model decisions (built)

int, float, date (iso/us/eu/any), bool (true/false/1/0/yes/no/t/f/y/n), enum,
per-column schema by name or index, empty_ok (doubles as global "required"),
delimiter auto-detect, max_issues cap, unknown-declared-column detection
(a typo'd column can't masquerade as a pass → marks the result invalid).

## Out-of-model / considered, not built

- **Per-column min/max value, min/max length, regex** (ConvertCSV) — real range/format
  checks beyond declared *type*; scope creep for a type validator. Regex has a dedicated
  niche; listed, not built.
- **Business formats: email / zip / phone / SSN / NPI-Luhn** (ConvertCSV, Flipper email) —
  gizza already ships dedicated tools (`email-validator`, `luhn-validate`, …); wrong to
  duplicate inside a type validator.
- **Missing-value sentinel list (NULL/NA/-/nil…)** — `empty_ok` covers blank cells; a
  configurable sentinel vocabulary is a nice-to-have, deferred (would bloat the schema).
- **Auto-fix / coerce values** (ConvertCSV UPPER/lower/trim, "apply fixes") — this tool is
  report-only by design; coercion belongs to `csv-cleaner`.
- **Custom date-format tokens / epoch dates** (ConvertCSV) — `any` + the 3 presets cover the
  common cases; a full token parser is deferred.
- **Reusable saved rule files (JSON schema import/export)** — the `types` string IS the
  portable schema (paste/share/deep-link); a file format is out of scope.

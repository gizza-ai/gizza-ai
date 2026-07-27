# enum-domain-check — competitor analysis (2026-07-26)

**Tool function:** flag CSV column cells whose value falls outside an allowed set of
categories (a "domain check" / controlled-vocabulary / allowed-values / categorical
membership check).

## Competitors skimmed

Search: "validate CSV column values against allowed set of categories domain check data
quality tool". Skimmed the top real tools (paraphrased — no copy/branding reproduced):

1. **ConvertCSV — CSV Validator** (convertcsv.com/csv-validator). Per-field validation
   rules including an explicit **"allowed values"** rule alongside required, min/max,
   min/max length, type (ZIP, US phone, US state, email, SSN, NPI), and regex. You define
   which values are acceptable per column; violations are listed. Confirms allowed-values /
   domain membership is a table-stakes rule in this space.
2. **Great Expectations — `expect_column_values_to_be_in_set`** (the canonical data-quality
   framework's own expectation for this exact check). Takes a `value_set` (the allowed
   categories) plus a `mostly` tolerance, and reports an **`unexpected_list`** and
   **`partial_unexpected_counts`** (distinct offending values with how many times each
   occurred). The distinct-unexpected-with-counts summary is the killer feature for domain
   checks — it turns "2 bad rows" into "you have a typo: `activ` appears 37 times".
3. **CleanMyExcel / cleanmyexcel.io — data validator.** Free online validator that detects
   invalid formats, wrong data types, and **out-of-range / not-in-allowed-set values** for
   pasted Excel/CSV, then flags the offending cells. Confirms the paste-CSV → flag-cells UX.

(csvlint / RFC-4180 structural validators were also surfaced but only check CSV *structure*,
not value membership — a different, already-built tool: `csv-structure-validator`.)

## Table-stakes params / behaviors (each tagged in/out-of-model)

| Capability | Decision |
| --- | --- |
| Specify the allowed set of categories | **in-model** — `allowed` param (comma / tag-list) |
| Select column by header name or index | **in-model** — `column` (mirrors regex-column-validate) |
| Case-insensitive membership | **in-model** — `ignore_case` |
| Trim whitespace before comparing | **in-model** — `trim` |
| Blank / null handling policy | **in-model** — `allow_blank` |
| Delimiter (comma/tab/;/\| + auto-detect) | **in-model** — `delimiter` |
| List offending cells (row, line, value) | **in-model** — `invalid_rows` |
| **Distinct unexpected values + counts** | **in-model** — `unexpected_values` (the GE differentiator) |
| Cap the issue list | **in-model** — `max_issues` (slider) |
| Text or structured JSON report | **in-model** — `output` |
| `mostly` / percentage tolerance (pass if ≥ X% in set) | **out-of-model** — not built; the report gives exact valid/invalid counts so the caller can apply any threshold themselves. Noted, not dropped silently. |
| Multi-column / full-schema validation in one run | **out-of-model** — this is the focused single-column tool; `data-validator` covers multi-rule schemas (see FAQ). |
| Business-format rules (ZIP/phone/SSN) | **out-of-model here** — covered by `format-validator` / `regex-column-validate`; this tool is membership-only. |

## Design decisions

- Mirrors the existing narrow single-column CSV validator family (`regex-column-validate`,
  `csv-column-type-validator`, `date-column-validate`) for a consistent UX: same
  `data`/`column`/`has_header`/`allow_blank`/`delimiter`/`max_issues`/`output` param shape.
- **Not a dup of `data-validator`** (which has a general `enum=a|b|c` rule among many, over
  CSV *or* JSON, driven by a rules mini-language). This is the focused, one-click,
  paste-a-column-and-a-vocabulary tool — same relationship `regex-column-validate` has to
  `data-validator`'s `regex=` rule.
- The `allowed` field uses the page's `kind = "tag-list"` control (comma-joined value) so
  each category is a removable pill — matching competitors' "allowed values" chip UX.
- Ships the distinct **unexpected values with counts** summary (GE's
  `partial_unexpected_counts`) because it is the single most useful output for finding data
  typos, which plain per-row lists bury.

Paraphrased only; no competitor copy, branding, or trademarks reproduced.

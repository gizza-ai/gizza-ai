# numeric-range-check — Competitor Analysis (2026-07-31)

Scope: browser-local tool that flags CSV numeric values outside an expected min/max range. Our tool is browser-local, wasm, no-account, no-server, report-only. All competitor descriptions below are paraphrased; no vendor copy or branding is reproduced.

## Competitors (top 3)

### 1. Great Expectations — `expect_column_values_to_be_between` — https://greatexpectations.io/legacy/v1/expectations/expect_column_values_to_be_between/

- What it does: A per-column data-quality assertion (Python library / data-validation framework) that checks whether every value in a chosen column falls inside a numeric (or comparable) lower/upper bound and reports which rows fail.
- Params/options:
  - `min_value` / `max_value` — the bounds; either may be omitted (None) to make the check a one-sided lower- or upper-bound test.
  - `strict_min` / `strict_max` — per-bound toggles to switch from inclusive (`>=`/`<=`) to strictly-exclusive (`>`/`<`). Bounds are independently configurable, so you can mix inclusive-low with exclusive-high.
  - `mostly` — tolerance float (0–1); the check still "passes" if at least that fraction of rows are in range, so a few outliers don't fail the whole column.
  - `parse_strings_as_datetimes` / `output_strftime_format` — coerce string cells to datetimes so range checks work on temporal columns.
  - Column is targeted by name.
- Defaults: bounds inclusive unless the strict flags are set; no default numeric bounds (caller supplies them); one column per expectation.
- Non-numeric handling: relies on comparable types; nulls are generally excluded from the evaluated set rather than counted as failures. It is not primarily a "flag the junk text" tool — it assumes typed columns.
- Input/output: input is a tabular dataset (pandas/Spark/SQL-backed batch), not a raw CSV upload per se. Output is a structured validation result object exposing a success boolean plus metrics such as an unexpected-value count and a list/sample of the offending values — machine-readable and renderable into HTML "Data Docs".
- UX worth emulating: (a) independent per-bound strict/inclusive toggles; (b) one-sided bounds by leaving a bound blank; (c) a "mostly"/tolerance threshold so a column can pass with a small failure rate; (d) returning both a count and a sample of unexpected values, not just pass/fail.
- SEO angle / positioning: positioned as reproducible "data quality / data testing" for pipelines; ranks for expectation names, dbt-expectations equivalents, and "validate column values between".

### 2. Spreadsheet data validation (Google Sheets "is between" / Excel "Data Validation > Whole/Decimal > between") — https://support.google.com/docs/answer/3378149

- What it does: A built-in cell/range rule in end-user spreadsheets that constrains a selected range to numbers within a min–max window, rejecting or warning on out-of-range entries.
- Params/options:
  - Criterion dropdown offering `between`, `not between`, plus the individual comparators (`greater than`, `>=`, `less than`, `<=`, `equal`, `not equal`) — so exclusive/one-sided behavior is achieved by choosing a different operator rather than a strict toggle.
  - Two boxes for the min and max bounds.
  - Invalid-entry behavior: show a warning (allow but flag) vs. reject the input outright.
  - Custom-formula escape hatch, e.g. combining an is-number test with `>=`/`<=` comparisons to both enforce numeric type and the range in one rule.
  - Excel equivalent additionally lets you pick Whole number vs. Decimal, and add an input prompt + custom error message.
- Defaults: `between` bounds behave inclusively; default action is typically to warn rather than hard-reject.
- Non-numeric handling: a plain number rule doesn't inherently validate that text is numeric — users add an `ISNUMBER`/whole-vs-decimal guard via the custom formula/number-type option.
- Input/output: input is the live grid; "output" is inline UI — red corner flags, on-hover warnings, or entry rejection — not a downloadable report.
- UX worth emulating: (a) a single operator dropdown that unifies between / not-between / one-sided comparisons; (b) warn-vs-reject choice (analogous to our flag-vs-ignore posture); (c) a human-readable custom error message; (d) whole-number vs decimal distinction.
- SEO angle / positioning: dominates "data validation google sheets / excel", "restrict number range in a cell", "drop-down and rules" tutorials — heavy how-to content targeting spreadsheet users.

### 3. pandas `Series.between()` (and the broader pandas/CSV validation idiom) — https://pandas.pydata.org/docs/reference/api/pandas.Series.between.html

- What it does: Returns a boolean mask marking which elements of a column lie between a left and right bound (`left <= x <= right`), the standard building block for numeric range validation in Python/CSV workflows (`pd.read_csv` then `.between`).
- Params/options:
  - `left` / `right` — the two bounds.
  - `inclusive` — a four-way selector: `both` (default), `neither`, `left`, or `right`, i.e. per-edge open/closed control in one argument.
  - Because it operates on a Series/DataFrame, you apply it per column and can combine masks across columns.
- Defaults: `inclusive="both"` (both bounds closed/inclusive).
- Non-numeric handling: NA/NaN evaluate to False (treated as out-of-range/failing); if a column is object-typed with junk text, comparisons can raise or mis-sort unless the caller coerces with `pd.to_numeric(errors="coerce")` first — the type-coercion step is on the user.
- Input/output: input is a DataFrame column loaded from CSV (delimiter, header, dtype all configurable in `read_csv`); output is a boolean Series you use to count, filter, or list failing rows — report format is whatever the user codes.
- UX worth emulating: (a) the single `inclusive` enum (`both`/`neither`/`left`/`right`) is a cleaner API than two separate strict flags; (b) the explicit `to_numeric(errors="coerce")` pattern maps directly to our non-numeric flag/ignore option; (c) trivially vectorized across columns.
- SEO angle / positioning: ranks for "pandas between", "filter values within range", "pandas validate numeric range CSV" — developer-tutorial traffic.

## Capability gap analysis vs our tool

Our tool params: data (CSV), columns (names or 1-based indices, or "all"), min, max, inclusive bounds toggle, delimiter (auto/comma/tab/semicolon/pipe), header toggle, empty_ok, non_numeric handling (flag/ignore), max_issues, format (text/json). Browser-local, wasm, no-account, no-server, report-only.

| Competitor feature | In/Out of model | Reason |
|---|---|---|
| Per-bound strict/exclusive (GX `strict_min`/`strict_max`) | **in-model** | Pure compare logic; split our single inclusive toggle into per-edge control. |
| `inclusive` four-way enum both/neither/left/right (pandas) | **in-model** | Same logic as above, cleaner param shape; no backend. |
| One-sided bounds — leave min or max blank (GX None bound) | **in-model** | Just make min/max optional in the compare. |
| `mostly` / failure-rate tolerance threshold (GX) | **in-model** | Compute fraction failing locally; pass/fail is a local comparison. |
| Return unexpected count + sample of offending values (GX) | **in-model** | We already emit issues; ensure JSON includes a total count and per-column tallies, capped by max_issues. |
| Whole-number vs decimal / integer-only rule (Excel) | **in-model** | Local numeric parse can reject non-integers optionally. |
| Warn-vs-reject action (Sheets/Excel) | **in-model (partial)** | We are report-only, so map to severity/flag levels in output rather than blocking entry. |
| Per-column different min/max in one pass (all competitors do this per column) | **in-model** | Support a column→range mapping instead of one global min/max for "all". |
| Custom human-readable error message per rule (Excel) | **in-model** | Optional message string echoed in the report. |
| Datetime/string-parsed range checks (GX `parse_strings_as_datetimes`) | **in-model (optional)** | Date parsing is browser-local; scope creep beyond "numeric" though. |
| HTML "Data Docs" rendered report (GX) | **out-of-model** | GX's rich hosted docs assume a project/artifact store; we stay text/json report-only (a styled HTML export could be a lighter in-model subset). |
| Live inline cell flagging in a grid (Sheets/Excel) | **out-of-model** | Requires a full editable spreadsheet UI/host app, not a report-only check. |
| Backend batch over Spark/SQL sources (GX) | **out-of-model** | Needs server/engine connections; contrary to browser-local wasm. |

## Recommendations

Concrete in-model improvements for our tool:

- Replace the single inclusive on/off toggle with a four-way `inclusive` option (both / neither / left / right), or add independent `strict_min` / `strict_max` flags, to match GX and pandas semantics.
- Make `min` and `max` individually optional so the tool supports one-sided lower- or upper-bound checks.
- Support per-column ranges (a column→{min,max,inclusive} mapping) rather than one global min/max, since every competitor validates per column.
- Add an optional tolerance/`mostly` threshold that yields an overall pass/fail per column while still listing outliers.
- Add an optional integer-only / whole-number rule alongside the range check (Excel parity).
- Ensure JSON output reports a total out-of-range count plus per-column tallies and a capped sample of offending values (row index, column, value), aligning with GX's unexpected count + sample pattern.
- Keep the non_numeric flag/ignore option and document it as the browser-local equivalent of pandas `to_numeric(errors="coerce")`.
- Optionally allow a custom per-rule message echoed into the report (Excel-style), and consider a styled HTML report export as an in-model subset of GX Data Docs.

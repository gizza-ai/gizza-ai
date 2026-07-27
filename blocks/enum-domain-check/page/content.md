## About this tool

A **domain check** (also called a controlled-vocabulary, allowed-values, or categorical
membership check) confirms that every value in a column is one of a known, agreed-upon set of
categories. It is one of the most common data-quality checks: a `status` column should only ever
hold `active`, `inactive`, or `pending`; a `country` column should only hold your supported ISO
codes. A single typo — `activ` instead of `active` — silently breaks downstream filters, joins,
and reports.

This tool takes CSV text, a target column, and the allowed set of categories, then flags every
cell whose value is **not** in the set. It reports how many cells were checked, how many were
valid, how many were invalid, the exact offending rows (with their line numbers), and — most
usefully — a summary of the **distinct unexpected values with a count of each**. That summary
turns "37 bad rows" into "you have one typo, `activ`, appearing 37 times", which is what you
actually need to fix the data.

### Worked example

Paste this CSV, set the column to `status`, and add `active`, `inactive`, and `pending` as the
allowed values:

```
name,status
Ada,active
Bo,activ
Cy,pending
De,active
```

The report flags row 2 — `activ` is not in the allowed set — and lists it under **Unexpected
values** as `"activ" ×1`, while confirming the other three rows are valid.

### Options

- **Ignore case** — off by default (an exact, case-sensitive match). Turn on to treat `ACTIVE`,
  `Active`, and `active` as the same category.
- **Trim surrounding whitespace** — on by default, so `" active "` matches `active`. Turn off to
  require an exact, unpadded value.
- **First row is a header** — on by default; the column is then chosen by header name. Turn off to
  select the column by its 0-based index.
- **Allow blank cells** — on by default (empty cells are skipped). Turn off to flag blank cells as
  invalid.
- **CSV delimiter** — auto-detects comma, tab, semicolon, or pipe from the first line; override it
  if detection guesses wrong.
- **Max issues to list** — caps how many offending rows and distinct unexpected values are listed
  (the total invalid count is always reported in full).
- **Output format** — a readable text report, or a structured JSON object for scripting.

Everything runs locally in your browser — your CSV is never uploaded.

## FAQ

<details>
<summary>How do I select the column — by name or by number?</summary>

Either. With **First row is a header** on (the default), type the header name (for example
`status`). With it off, type the 0-based index of the column instead (`0` is the first column,
`1` the second). A purely numeric value is always treated as an index, even when a header row is
present, so you can index into a file whose headers happen to be numbers.

</details>

<details>
<summary>What is the difference between this and a regex or type validator?</summary>

This tool checks **membership in a fixed set of categories** — the value must be exactly one of
the allowed strings. A regex validator checks that values match a **pattern** (like a ZIP code or
email shape), and a type validator checks that values are the right **data type** (integer, date,
boolean). Use a domain check when the valid values are a short, known list you can enumerate.

</details>

<details>
<summary>Why does it summarize distinct unexpected values with counts?</summary>

Because a plain row-by-row list buries the real problem. If one misspelling appears in hundreds of
rows, the per-row list is a wall of duplicates. The **Unexpected values** summary collapses that to
one line — `"activ" ×342` — showing at a glance whether you have one systematic typo or many
scattered bad values. It is the single most useful output for cleaning categorical data.

</details>

<details>
<summary>Can it check several columns or apply a percentage tolerance in one run?</summary>

No — this is the focused single-column tool: it validates one column against one allowed set and
reports exact valid/invalid counts. There is no built-in "pass if ≥ 95% are valid" tolerance, but
the report gives you the precise counts, so you can apply any threshold you like yourself. For
multi-column or multi-rule schema validation, use a general CSV validator instead.

</details>

## What this tool does

**CSV Column Mapping Suggest** compares two CSV files and recommends which source columns should map to which target columns before you run a diff, join, migration, or import. It scores each possible source→target pair with two signals: a normalized fuzzy header match and an optional value-overlap check from sampled data rows.

Use it when the same field appears under different names — for example `Email Address` in one export and `email` in another, or `Zip Code` and `postal_code`. The output is a one-to-one mapping with confidence scores, brief reasons, and lists of columns that were not mapped above your threshold.

## Worked example

**Source CSV**

```csv
First Name,Email Address,Zip Code
Ada,a@example.com,02139
Bo,b@example.com,94107
```

**Target CSV**

```csv
email,postal_code,first_name
a@example.com,02139,Ada
b@example.com,94107,Bo
```

With the default settings, the table output suggests:

```text
Source column | Target column | Score | Reason
--- | --- | ---: | ---
Email Address | email | 0.720 | header 0.53, value 1.00
First Name | first_name | 1.000 | header 1.00, value 1.00
Zip Code | postal_code | 0.640 | header 0.40, value 1.00

Unmapped source columns: (none)
Unmapped target columns: (none)
```

Raise the threshold when you only want strong matches; lower it when you would rather see weaker candidates for manual review.

## FAQ

<details>
<summary>Does this change either CSV?</summary>

No. It only reads the source and target CSV text and returns suggested column mappings. It does not rename, reorder, join, or upload any data. Use the suggestions in your import pipeline, diff tool, or join step.

</details>

<details>
<summary>How are the confidence scores calculated?</summary>

Each candidate pair gets a header score from normalized header tokens and character bigrams, plus a value score from the overlap of distinct sampled cell values. `header_weight` controls the blend: `1` means header-only, `0` means value-overlap only, and the default `0.6` gives headers slightly more weight.

</details>

<details>
<summary>What does the threshold do?</summary>

The threshold is the minimum combined score required before a mapping is suggested. A higher threshold leaves more columns unmapped for manual review. A lower threshold surfaces more tentative matches, which can be useful with messy exports but should be checked carefully.

</details>

<details>
<summary>Can it handle tabs, semicolons, or pipe-delimited data?</summary>

Yes. Choose `tab`, `semicolon`, or `pipe` in the delimiter control when both inputs use that separator. The delimiter applies to both source and target CSVs in this version.

</details>

## Limits & edge cases

- Both inputs must be CSV text. The delimiter setting is shared by both inputs.
- The tool makes **one-to-one** suggestions: once a source and target are paired, neither is reused.
- `sample_rows` is capped at 500. Use `0` for header-only matching when the files contain no representative sample values.
- Header matching is heuristic, not ML. Domain-specific synonyms such as `zip` ↔ `postal` may need value overlap or manual review.
- Empty inputs and malformed CSV rows return clear parse errors instead of partial mappings.

## About this tool

Category Canonicalize cleans categorical columns when the same value appears under several spellings, abbreviations, capitalization styles, or whitespace variants. Paste a CSV/TSV table or a one-value-per-line list, supply a mapping such as `USA|U.S.A.|us => United States`, and the tool rewrites only the selected column(s).

Use the suggestions output as a review pass: it lists values not covered by the mapping, how often they occur, and the closest canonical value. Accept the suggestions you trust by adding them to the mapping, then rerun to produce the final table. All matching is deterministic and based on the vocabulary you provide; this is for controlled normalization, not unsupervised clustering.

### Worked example

Input data:

```csv
country,n
USA,1
u.s.a.,2
Canadaa,3
Brazil,4
```

Mapping:

```text
USA|U.S.A.|us|united states => United States
Canada|CAN => Canada
```

With `column = country`, `header = true`, and `output = csv`, the USA variants become `United States`, canonical `Canada` stays canonical, and uncovered `Canadaa`/`Brazil` remain available for review or fuzzy handling.

## Limits and edge cases

- Input is capped at 2 MB and mapping text at 200 KB so browser runs stay responsive.
- The mapping is explicit: a bare line declares an accepted canonical, while `variant => canonical` rewrites variants to that value.
- Matching can ignore case and collapse whitespace; punctuation is not stripped unless you list that variant.
- Fuzzy matching compares unmatched values to supplied canonicals with an edit-distance ratio. It is a suggestion aid, not a semantic model.
- Header rows are never rewritten. Select multiple columns with comma-separated header names or 1-based indexes.

## FAQ

<details>
<summary>Can it discover clusters without a mapping?</summary>

No. This tool applies a supplied vocabulary and suggests the nearest supplied canonical for uncovered values. Use it when you already know the allowed labels and want an auditable cleanup pass.

</details>

<details>
<summary>How do I review fuzzy matches before changing the table?</summary>

Choose the `suggestions` output. It returns a CSV with each uncovered value, its count, the nearest canonical, and the similarity score. Add accepted rows to your mapping and rerun, or switch unmatched values to `fuzzy` once the threshold is conservative enough.

</details>

<details>
<summary>What mapping separators are accepted?</summary>

Use `=>`, `->`, `=`, a tab, a comma, or a semicolon between variants and canonical values. Use `|` to list several variants for the same canonical, for example `NY|N.Y.|new york => New York`.

</details>

<details>
<summary>What happens to values not covered by the mapping?</summary>

Pick the policy that fits your workflow: keep originals, blank them, stop with an error, or apply the closest fuzzy suggestion when it reaches the threshold.

</details>

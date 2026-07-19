## About this tool

This data anonymizer applies the classic k-anonymity workflow to a pasted CSV — the same
three-step model used by research de-identification software. **Identifier columns** (names,
emails, SSNs) are replaced outright with a fixed label. **Quasi-identifier columns** — the
attributes that don't identify anyone alone but do in combination, like age + zip code +
gender — are *generalized*: numeric values fall into fixed-width bins (`34` → `30-39`), ISO
dates collapse to the year (`1987-04-12` → `1987`), and text keeps only its first few
characters (`London` → `Lon***`). Rows are then grouped by their generalized quasi-identifier
combination, and the report tells you the **achieved k** — the size of the smallest group,
i.e. how many records every person hides among.

Set a **target k** to see which share of rows still falls below it, or enable **suppression**
to drop those rows so the remaining dataset is guaranteed k-anonymous. Need finer control?
Give any quasi-identifier its own generalization level with a `:level` suffix — `zipcode:100`
bins that column by 100 while ages keep the default width. Naming a **sensitive column**
(e.g. diagnosis) adds the distinct l-diversity: the minimum number of different sensitive
values inside any group, a guard against everyone in a group sharing the same diagnosis.

### Worked example

Input (quasi-identifiers `age,zipcode:100,gender`, identifier `name`, sensitive `diagnosis`):

```
name,age,zipcode,gender,diagnosis
Ada,34,13053,F,Flu
Bea,38,13068,F,Cold
Cal,52,14850,M,Flu
Dan,57,14853,M,Fever
```

Output:

```
name,age,zipcode,gender,diagnosis
[REDACTED],30-39,13000-13099,F,Flu
[REDACTED],30-39,13000-13099,F,Cold
[REDACTED],50-59,14800-14899,M,Flu
[REDACTED],50-59,14800-14899,M,Fever

K-anonymity report
Data rows: 4 (0 suppressed)
Quasi-identifiers: age, zipcode, gender
Redacted identifiers: name
Equivalence classes: 2 (smallest 2, largest 2)
Achieved k = 2 — every row is indistinguishable from at least 1 other row(s)
Target k = 2: MET — no rows fall below the target
Distinct l-diversity on 'diagnosis': l = 2
```

### Limits and edge cases

- At most **10,000 data rows** per run (the header row doesn't count).
- Column types are auto-detected per quasi-identifier column: all-numeric → bins, all
  ISO dates (`YYYY-MM-DD` / `YYYY-MM`) → year, anything else → prefix masking. Codes with
  leading zeros (`01035`) are treated as text so the zeros survive.
- Bins for columns containing negative or fractional values use interval notation
  (`[-10,0)`); non-negative integer columns use the friendlier `30-39` style.
- Empty cells stay empty and form their own equivalence class.
- Suppression can remove every row when the target k is unreachable — the report says so
  rather than failing.

## FAQ

<details>
<summary>What is k-anonymity and what counts as a "good" k?</summary>

A dataset is k-anonymous when every row shares its quasi-identifier combination (its
*equivalence class*) with at least k-1 other rows, so no individual can be singled out among
fewer than k records. `k = 1` means at least one person is unique and re-identifiable by
linking; `k = 2` is the bare minimum of protection, and public-release guidance often asks
for `k = 5` or more. Raise k by widening bins, keeping fewer text characters, or suppressing
the outlier rows.

</details>

<details>
<summary>What is the difference between identifier and quasi-identifier columns?</summary>

Direct identifiers (name, email, phone, SSN) point at one person on their own, so they are
replaced entirely with the label — no analytic value is preserved. Quasi-identifiers (age,
zip code, gender, birth date) look harmless alone but combine into a fingerprint; they are
generalized just enough to blur that fingerprint while keeping the data usable. Columns you
list in neither role pass through unchanged.

</details>

<details>
<summary>How do I give each column its own generalization level?</summary>

Append `:level` to any entry in the quasi-identifier list. For a numeric column the level is
the bin width (`zipcode:100` groups 13053 into `13000-13099`); for a text column it is the
number of characters kept (`city:2` turns `London` into `Lo****`). Columns without a suffix
use the global "bin width" and "characters kept" settings.

</details>

<details>
<summary>Does anonymizing quasi-identifiers guarantee privacy?</summary>

No — k-anonymity protects against linking on the quasi-identifiers you selected, but it
cannot help if you missed one, and a group can still leak information when everyone in it
shares the same sensitive value. Name a sensitive column to check the distinct l-diversity
(`l = 1` means some group is homogeneous), and treat the result as one layer of a broader
de-identification review, not a certification.

</details>

<details>
<summary>Why did my zip codes turn into ranges instead of 130** prefixes?</summary>

Columns whose values all parse as numbers are binned numerically, so `13053` with a width of
100 becomes `13000-13099` — the same grouping a two-digit truncation gives, just written as a
range. Codes with leading zeros (`01035`) don't survive numeric parsing and are therefore
kept as text and prefix-masked (`010**`). Either way, rows with the same prefix land in the
same equivalence class.

</details>

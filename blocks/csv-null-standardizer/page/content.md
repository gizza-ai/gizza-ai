## About this tool

Open enough real CSV exports and you will find the same column expressing "no data" four different ways: an empty cell here, `NA` there, `#N/A` from a spreadsheet, `null` from a database dump, and a lone `-` typed by whoever maintained the sheet by hand. Every downstream step then needs its own list of exceptions — a `na_values=` argument, a `CASE WHEN` in the load script, a find-and-replace before the import.

This tool collapses all of that into one rule. Paste the table, pick the single representation you want, and every cell that means "missing" is rewritten to it. Everything else is left exactly as it was: real values are copied byte-for-byte (padding included), the header row is never touched, quoted fields keep their quoting, ragged rows keep their length, and the field separator round-trips unchanged.

It standardizes the **token**, not the data. Nothing is guessed, averaged, or filled in — if you need means, medians, or nearest-neighbour fills, that is imputation and a different job. If you first want to know *how much* is missing and *where*, profile the table before you normalize it.

### Worked example

Input (delimiter `comma`, replace with `NULL`, defaults everywhere else):

```text
id,score,notes
1,42,ok
2,NA,
3,null,-
4,n/a,NaN
```

Output:

```text
id,score,notes
1,42,ok
2,NULL,NULL
3,NULL,NULL
4,NULL,NULL
```

Note what did *not* change: the header stays `id,score,notes` even though a header cell could match a token, and `42`/`ok` are untouched. `n/a` matched `N/A` because matching is case-insensitive by default, and the empty cell on row 2 matched because blank cells count as missing.

### Controls

- **Delimiter** — `comma` (default), `tab`, `semicolon`, `pipe`, any single character, or `auto` to sniff the separator from the first line. The output always uses the same separator as the input.
- **Tokens that count as missing** — the comma-separated vocabulary, pre-filled with `NA`, `N/A`, `N.A.`, `#N/A`, `#N/A N/A`, `#NA`, `NULL`, `NIL`, `NaN`, `None`, `<NA>`, `-`, `--`, and `?`. Edit it in place: add a project sentinel like `-999`, or delete entries you use as real values. Clear it entirely to standardize *only* blank cells.
- **Replace missing cells with** — the one representation you want. Leave it blank for an empty cell; type `NULL`, `NA`, `NaN`, or `\N` (the sentinel a Postgres `COPY … WITH (FORMAT csv, NULL '\N')` load expects).
- **Also standardize blank / whitespace-only cells** — on by default. Turn it off to convert the listed tokens and leave already-empty cells empty.
- **Match tokens case-sensitively** — off by default, so a single `NULL` entry also catches `null` and `Null`. Turn it on when case carries meaning.
- **Ignore whitespace around a cell when matching** — on by default, so ` NA ` is recognised. This only affects matching; a cell that is *not* missing keeps its padding.
- **First row is a header** — on by default. The header is copied through untouched and supplies the names used by the column filter.
- **Only these columns** — comma-separated column names (needs a header) or 1-based positions, e.g. `score,notes` or `2,4`. Blank means every column, and you never have to split the file up to normalize one field.
- **Output quoting** — `minimal` quotes only fields that require it, `always` quotes every field including the replacement token, `never` writes bare fields. Keep `minimal` when your replacement is a loader sentinel like `\N`, because a quoted `"\N"` is read as the literal two-character string.

### Limits and edge cases

The table is capped at 5,000,000 bytes. A row shorter than the header stays short — missing trailing cells are not invented, because writing a value into a column the row never had would change its shape. Because the token list itself is comma-separated, a token that literally contains a comma cannot be expressed. Choosing `never` for output quoting can produce ambiguous CSV when a value contains the separator or a newline, so keep `minimal` unless a downstream reader demands otherwise. Output rows are terminated with a single newline (`\n`), and the result always ends with one. Everything runs locally in your browser — the table is never uploaded.

## FAQ

<details>
<summary>Which tokens count as missing by default?</summary>

`NA`, `N/A`, `N.A.`, `#N/A`, `#N/A N/A`, `#NA`, `NULL`, `NIL`, `NaN`, `None`, `<NA>`, `-`, `--`, and `?` — plus any cell that is empty or whitespace-only. Matching ignores case and surrounding whitespace by default, so one `NULL` entry covers `null`, `Null`, and ` NULL `. The list is an editable field, not a fixed rule: add your own sentinels or delete the ones that are real values in your data.

</details>

<details>
<summary>A real value in my data is a dash — how do I protect it?</summary>

Two ways. Delete `-` from the **Tokens that count as missing** list so a dash is never treated as missing anywhere, or leave the list alone and use **Only these columns** to restrict the rewrite to the columns where a dash really does mean "no data". The header row is protected either way, so a column *named* `-` or `NA` always survives.

</details>

<details>
<summary>How do I produce a file a Postgres COPY can load?</summary>

Set **Replace missing cells with** to `\N` and load with `COPY table FROM 'file.csv' WITH (FORMAT csv, HEADER, NULL '\N')`, and leave **Output quoting** on `minimal` — Postgres only honours the NULL sentinel when it is unquoted.

If you were hoping to keep an empty *string* distinct from a NULL, that distinction cannot survive this tool: a bare empty cell and a quoted `""` both read as the same empty value on the way in, so there is nothing left to tell apart on the way out. Use a sentinel like `\N` for the missing cells instead, and anything that is still blank is your empty string.

</details>

<details>
<summary>Will this fill in the missing values for me?</summary>

No, and that is deliberate. This tool only makes the *representation* consistent; it never invents data. Substituting a mean, median, most-frequent value, or a nearest-neighbour estimate is imputation, which changes your dataset's statistics and belongs in a dedicated imputation step you can review separately.

</details>

<details>
<summary>Does it handle TSV, semicolon, and pipe files?</summary>

Yes. Choose `tab`, `semicolon`, or `pipe` by name, type any single character, or pick `auto` to detect the separator from the first line (it counts candidates outside quoted fields and prefers a comma on a tie). Whatever separator comes in is what goes back out, so the file shape is preserved.

</details>

## About this tool

CSV Join merges two CSV files into one by matching rows on a **key column** — the
same idea as a SQL `JOIN` or a spreadsheet `VLOOKUP`, but on whole tables at once.
Paste your left and right CSVs, name the key column on each side, pick a join type,
and get a single combined table back.

Four join types cover the usual cases:

- **Inner** — keep only rows whose key appears in **both** files.
- **Left** — keep every left row; fill blank cells where the right file has no match.
- **Right** — keep every right row; fill blanks on the left side.
- **Outer** (full outer) — keep **every** row from both files, blanks where a side is missing.

The key is matched on **values**, not column names, so the two key columns can be
named differently (`user_id` on the left, `uid` on the right). Reference a key by
its header name or by 1-based position. The output keeps the full left header, then
appends every non-key column from the right file; if a right column name collides
with a left one it gets a `_right` suffix so nothing is silently overwritten.

Everything runs locally in your browser — your data is never uploaded.

## FAQ

<details>
<summary>What if the two files use different names for the key column?</summary>

That's fine — the join matches on the **values** in the key columns, not their
names. Set the left key (e.g. `user_id`) and the right key (e.g. `uid`)
independently. Leave the right key blank to reuse the left key's name or index.

</details>

<details>
<summary>What's the difference between inner, left, right, and outer joins?</summary>

**Inner** keeps only keys present in both files. **Left** keeps all left rows
(blank right cells when unmatched), **right** keeps all right rows, and **outer**
keeps every row from both sides, padding the missing side with blanks. Inner is the
default.

</details>

<details>
<summary>What happens when both files have a column with the same name?</summary>

The output keeps the full left header first, then appends the right file's non-key
columns. If a right column's name already exists in the output, it's suffixed with
`_right` (e.g. `name` and `name_right`) so both values are preserved.

</details>

<details>
<summary>What if a key value appears more than once?</summary>

Duplicate keys produce a Cartesian product for that key — one output row per
matching left×right pair — exactly like a SQL join. Two left rows and three right
rows sharing a key yield six joined rows.

</details>

<details>
<summary>Can I join on a column by position instead of by name?</summary>

Yes. Enter a 1-based column index (e.g. `1` for the first column) instead of a
header name for either key. This is handy when your headers are awkward or absent.

</details>

<details>
<summary>Does the delimiter have to be a comma?</summary>

No. Set the delimiter to `,`, `tab`, `;`/`semicolon`, `|`/`pipe`, or any single
character. The same delimiter is used to parse both inputs and to write the result.

</details>

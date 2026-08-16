## About this tool

A header row written by a human is not a set of identifiers. It has spaces, currency symbols and units in brackets, a stray capital, a column somebody left blank, a year at the front, and — the one that actually costs you data — two different columns that both ended up called `Notes`. Every downstream step then has to cope: a database rejects `Total ($)`, a dataframe gives you `df['First Name']` instead of `df.first_name`, and a join keyed on a name that quietly became the *second* `notes` drops rows without telling you.

This tool rewrites that one row and nothing else. Paste the table, pick a casing, and the header comes back as valid identifiers: punctuation and spacing collapsed to single separators, accents transliterated, blank headers named after their position, digit-leading names repaired, and every collision deduplicated so no two columns share a label. Data rows are passed through untouched — no trimming, no re-typing, no row deduplication — quoted fields keep their quoting, and the field separator round-trips unchanged.

It renames **columns**, not values. If your cells are the mess, clean those separately; if you want to see the renames before you trust them, switch the output to the rename map and read every change as an `original,sanitized` pair.

### Worked example

Input (defaults everywhere: `snake`, ASCII on, underscore prefix, no length cap, dedupe by counting up):

```text
First Name, Total ($) ,2024 Revenue,,Notes,Notes
Ada,10,120,x,first,second
```

Output:

```text
first_name,total,_2024_revenue,column_4,notes,notes_2
Ada,10,120,x,first,second
```

Six problems fixed in one pass: the space in `First Name`, the currency symbol and padding in ` Total ($) `, the leading digit in `2024 Revenue`, the blank fourth header, and the duplicate `Notes` — the second of which became `notes_2` rather than overwriting the first. The data row is byte-for-byte what went in.

Switch **Output** to the rename map and the same input returns the audit trail instead:

```text
original,sanitized
First Name,first_name
 Total ($) ,total
2024 Revenue,_2024_revenue
,column_4
Notes,notes
Notes,notes_2
```

### Controls

- **Delimiter** — `,` (default), `tab`, `semicolon`, `pipe`, any single character, or `auto` to sniff the separator from the header line. Whatever comes in is what goes out.
- **Identifier style** — `snake_case` (default), `camelCase`, `PascalCase`, `kebab-case`, `SCREAMING_SNAKE`, `lowercase`, or `preserve`. The first five read `FirstName` as two words; `lowercase` and `preserve` deliberately do not split a CamelCase run, so `HTTPStatusCode` stays one token.
- **Transliterate Unicode to ASCII** — on by default: `Año` → `ano`, `Größe` → `grosse`, `名前` → `ming_qian`. Turn it off to keep the original letters, which is fine for a quoted SQL identifier or a dataframe column but not for a bare identifier.
- **Names starting with a digit** — an unquoted SQL identifier cannot begin with one. Prefix an underscore (default), prefix `col`, or keep it and quote the name yourself.
- **Max name length** — `0` means no limit. Use `63` for PostgreSQL, whose identifiers are truncated at 63 bytes; `300` is BigQuery's ceiling and this tool's maximum. Truncation cuts any separator left dangling at the end, and a deduplication suffix always stays inside the cap — the base name gives up characters to make room, so a capped `customer_lifetime_value` pair becomes `customer_lif` and `customer_l_2`.
- **Name for blank headers** — the base used when a header cell is empty or nothing but punctuation. The column's 1-based position is appended, so the default gives `column_2`, `column_4`.
- **When two names collide** — count up (`total`, `total_2`, `total_3`, the default), use the duplicate column's own 1-based position (`total`, `total_3`), or allow the collision if your reader genuinely keeps duplicate columns.
- **Output** — the whole table with the new header (default), just the cleaned header row, or the `original,sanitized` rename map.

### Limits and edge cases

The table is capped at 5,000,000 bytes and names at 300 characters. Only row 1 is rewritten — there is no "no header" mode, because a table without a header has nothing to sanitize. Collisions are matched exactly, so under `preserve` a `Total` and a `total` are two distinct names even though PostgreSQL would fold them together; pick a lowercasing style if that matters. Reserved words are left alone on purpose: the list differs across PostgreSQL, MySQL, BigQuery and Snowflake, so a built-in list would rename legitimate columns for most users. Output rows are terminated with a single newline (`\n`) and the result always ends with one. Everything runs locally in your browser — the table is never uploaded.

## FAQ

<details>
<summary>Why does the tool rename the second duplicate instead of dropping it?</summary>

Because dropping it loses data silently. Two source columns that both clean to `total` are two different columns; if one simply disappeared, or both were written under the same name, whichever value your reader kept would depend on its internal ordering. Suffixing the second one to `total_2` keeps both, makes the ambiguity visible, and is the same convention the standard dataframe-cleaning libraries use.

Watch for the follow-on trap: if you later join on `total` and the column you meant is now `total_2`, the join key silently misses. Run the tool once with **Output** set to the rename map, read the pairs, and fix the source header if two columns should never have shared a name.

</details>

<details>
<summary>Which style should I pick for SQL, and is the result really safe to use unquoted?</summary>

`snake_case` with the default underscore prefix for digit-leading names, plus a **max name length** of `63` if you are loading into PostgreSQL. That combination produces names made only of lowercase letters, digits and underscores, never starting with a digit and never longer than the identifier limit — which is exactly what an unquoted identifier is allowed to be.

The one thing it does not do is avoid reserved words. A column honestly called `Order` becomes `order`, which needs quoting in most dialects. That is deliberate: reserved lists differ per database, so a built-in list would silently rename columns that were fine. Quote the identifier, or rename that column yourself before you import.

</details>

<details>
<summary>What happens to blank headers, and to a header that is only punctuation?</summary>

Both get a positional name. A blank fourth column becomes `column_4`, and a header of `!!!` or `---` becomes `column_3` in position three, because after punctuation is stripped there is nothing left to build a name from. Change the base with **Name for blank headers** — set it to `field` or `unnamed` and you get `field_4`, `unnamed_4`. The position is always appended, so two blank headers can never collide with each other.

</details>

<details>
<summary>Does it change my data, drop rows, or fix the delimiter?</summary>

No. Only row 1 is rewritten. Data rows are re-emitted exactly as they were parsed — padding kept, values untouched, ragged rows still ragged, quoted fields still quoted where the CSV grammar needs it — and the separator you came in with is the separator you leave with. Cleaning cell values, removing duplicate *rows*, standardizing missing-value tokens, and validating column types are each a separate job with its own tool.

</details>

<details>
<summary>How do I clean headers that came from a TSV or a semicolon file?</summary>

Choose `tab`, `semicolon` or `pipe` by name, type any single character, or pick `auto` to detect the separator from the header line — it counts candidates outside quoted fields and prefers a comma on a tie. The output uses the same separator, so a TSV stays a TSV. If you only want the names, set **Output** to the cleaned header row and paste that line straight over the original.

</details>

<details>
<summary>My headers are in another language — will they survive?</summary>

With **Transliterate Unicode to ASCII** on (the default) they are folded to their closest ASCII spelling: `Año` → `ano`, `Größe` → `grosse`, `Ünit Price` → `unit_price`, `名前` → `ming_qian`. That is what you want for a bare SQL identifier or a filename.

Turn it off and the letters are kept as-is — `año` stays `año` — while spacing and punctuation are still normalized. That is valid for dataframe columns and for quoted SQL identifiers, but not for unquoted ones. Note that transliteration can create collisions that did not exist before (`Größe` and `Grosse` both become `grosse`); the deduplicator catches those and the rename map shows you where they happened.

</details>

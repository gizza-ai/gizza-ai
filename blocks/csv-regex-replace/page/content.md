## About this tool

Every spreadsheet eventually needs one surgical edit repeated a few thousand times: strip the currency symbols out of `amount`, turn `Lovelace, Ada` into `Ada Lovelace`, flatten `(555) 010-1234` to digits, blank out the cells that say exactly `NA`. The obvious move — open the file in an editor and run a regex over the whole thing — is also the one that quietly corrupts it. A pattern loose enough to catch your values is loose enough to eat a delimiter, swallow a quote character, or rewrite the same string in the one column that had to keep it.

This tool does the same job the safe way round. The CSV is **parsed first**, and the pattern is applied to each decoded cell value on its own. It cannot match across a comma, cannot see the quote characters that delimit a field, and cannot split a quoted cell that contains an embedded newline. When a replacement introduces a comma, a quote or a line break, the output is re-quoted automatically. Row order, column count and the delimiter come back exactly as they went in.

Then you say **where**. Leave the column box blank and every column is in scope; type `email`, `3`, or `2-4` — or all three at once — and everything else is copied through byte-for-byte.

### Worked example

Input, with `name` as the only column in scope, pattern `(\w+), (\w+)` and replacement `$2 $1`:

```text
name,city
"Lovelace, Ada",Paris
"Hopper, Grace",Boston
```

Output:

```text
name,city
Ada Lovelace,Paris
Grace Hopper,Boston
```

Two things happened that a raw-text regex gets wrong. The comma the pattern matched is the one *inside* the quoted cell, not the field separator — the tool never saw the separator as text at all. And because the rewritten values no longer contain a comma, the quotes around them are dropped: quoting is re-derived from the result, not carried over.

Switch **Output** to the audit report and you get counts instead of data:

```text
column,cells_changed,replacements
name,2,2
TOTAL,2,2
```

That is the safe way to try a rule on a file you care about — check the number matches what you expected, then switch back to the rewritten CSV. **Only the rows that changed** is the middle ground: the header plus the affected rows, nothing else.

### Capture groups and the replacement

In the default regex mode the replacement expands capture references:

- `$1`, `$2`, … — numbered groups; `${1}` is the same thing with an explicit boundary.
- `${name}` — a group captured by `(?<name>...)`.
- `$0` — the whole match.
- `$$` — a literal dollar sign.
- Blank — **deletes** every match, which is how `[^0-9]` turns a formatted phone number into digits.

Write `${1}x` rather than `$1x` when a group reference is followed by a letter, digit or underscore: `$1x` is read as a group *named* `1x`, which does not exist and expands to nothing. Switch to **Literal text** mode when the thing you are searching for is a pasted value full of regex punctuation — the pattern is then escaped for you, and the replacement is inserted verbatim, dollar signs and all.

### Controls

- **Find / Replace with** — the pattern and its replacement, as above.
- **Columns** — blank or `*` for every column; otherwise header names, 1-based indices and inclusive `2-4` ranges, comma-separated and mixable (`name,3,5-7`). Names match exactly first, then case-insensitively.
- **Pattern mode** — regular expression (default), or literal text.
- **Match** — anywhere inside a cell (default), or the entire cell only. Whole-cell anchors the pattern with `\A`/`\z`, so `NA` clears a cell that says exactly `NA` and leaves `NAME` alone.
- **Ignore case (i)**, **Multiline (m)**, **Dot matches newline (s)** — the three standard regex flags. `m` and `s` matter here because a quoted CSV cell may legally contain line breaks.
- **Replace every match in a cell** — on by default, the equivalent of a `g` flag. Off replaces only the first match in each cell.
- **First row is a header** — on by default: the header's names can be used in **Columns**, and the header itself is protected from the rule unless you tick **Also rewrite the header row**.
- **Delimiter** — `auto` (default) sniffs it from the first line, counting candidates outside quotes with comma winning a tie; or `comma`, `tab`, `semicolon`, `pipe`, or any single character. The output uses the same separator.
- **Output quoting** — minimal (default), always, or everything except numbers.
- **Output** — the rewritten CSV, only the changed rows, or the per-column audit report.

### Limits and edge cases

The table is capped at 5,000,000 bytes. Matching uses the Rust `regex` engine, which has **no backreferences and no lookaround** — that restriction is what guarantees linear-time matching, so no pattern you can type here can hang the page. Ragged rows are preserved rather than padded, one rule runs per pass (chain a second run for a second rule), and a match that produces identical text is counted as a replacement but not as a changed cell, which is why the audit report has both columns. Everything runs locally in your browser; the table is never uploaded.

## FAQ

<details>
<summary>Why not just run a regex over the CSV file in my editor?</summary>

Because the file is not text as far as your data is concerned — it is an encoding of a table, and the two disagree in exactly the places that hurt. A pattern like `,` matches both the field separator and the comma inside `"Lovelace, Ada"`. A pattern like `.*` runs past the closing quote of a field. A replacement that inserts a comma turns one column into two for that row only, and nothing complains until the import fails a week later.

Here the table is parsed into rows and cells before anything is matched, so the pattern only ever sees a decoded cell value: separators and the structural quote characters are simply not part of the text it can match. Output quoting is then re-derived from the resulting values — a cell whose new value contains the delimiter, a quote or a newline gets quoted, and a cell that no longer needs quotes loses them.

</details>

<details>
<summary>How do I reference a capture group in the replacement?</summary>

Use `$1`, `$2` for numbered groups and `${name}` for a group captured with `(?<name>...)`; `$0` is the whole match and `$$` is a literal dollar sign. So `(\d{3})-(\d{4})` with `$1$2` strips the dash, and `(?<user>[^@]+)@(?<host>.+)` with `${user} at ${host}` rewrites an address.

The one trap is a group reference followed by a word character. `$1x` is parsed as a group *named* `1x` — which does not exist, so it expands to an empty string and it looks like your group vanished. Write `${1}x` instead. If you want a literal `$1` in the output, either escape it as `$$1` or switch **Pattern mode** to Literal text, where the replacement is taken exactly as typed.

</details>

<details>
<summary>Can I use lookahead, lookbehind or backreferences?</summary>

No — `(?=...)`, `(?<=...)` and `\1` inside a pattern are rejected with an "invalid pattern" message rather than silently misbehaving. The engine is Rust's `regex`, which leaves them out deliberately: without them, every match is guaranteed linear in the length of the input, so no pattern typed into this page can lock up your browser the way a catastrophically backtracking one can elsewhere.

Most real uses have a direct rewrite. Instead of matching with a lookahead to protect surrounding text, capture that text in a group and put it back: `(\d)(?=%)` becomes `(\d)%` with replacement `$1%`. Instead of a lookbehind for a prefix, capture the prefix. And **Match: the entire cell only** covers the common "only if the cell is exactly this" case that people often reach for anchored lookaround to express.

</details>

<details>
<summary>What is the difference between whole-cell match and just anchoring with ^ and $?</summary>

Whole-cell wraps your pattern in `\A(?:...)\z`, which anchors to the start and end of the *value* and cannot be loosened. `^` and `$` anchor to the start and end of a line, and a CSV cell may contain several lines when it is quoted — and if you also turn on **Multiline**, `^`/`$` start matching at every internal line break, so an "anchored" pattern quietly matches in the middle of a cell.

The practical rule: use whole-cell for value remaps (blank out exactly `NA`, replace exactly `Y` with `true`), and use `^`/`$` when you genuinely mean line boundaries inside a multi-line cell. The wrapping is applied around your whole pattern, so alternations like `NA|N/A` behave as you would expect rather than binding the anchor to only the first branch.

</details>

<details>
<summary>How do I preview a rule before I trust it on a real file?</summary>

Run it twice. First set **Output** to the audit report: you get a `column,cells_changed,replacements` table with a `TOTAL` row and no data at all, so you can check that a rule meant to fix 12 rows reports 12 and not 4,000. Then set it to **Only the rows that changed** — the header plus just the affected rows — and read the actual before-and-after values for a handful of them.

`cells_changed` and `replacements` differ on purpose: a cell where the pattern matched three times counts once as a changed cell and three times as a replacement, and a match that produces text identical to what was there counts as a replacement but not a change. A large gap between the two numbers usually means the pattern is matching more loosely than intended.

</details>

<details>
<summary>Can I apply several find-and-replace rules at once?</summary>

Not in one pass — one rule runs per run, deliberately, because ordered multi-rule pipelines are where "it replaced the thing I had already replaced" bugs come from. Chain runs instead: take the output of the first pass, paste it back in, and change the pattern. Each pass is independent and auditable, and the report tells you what each one did.

If the rules apply to different columns they compose cleanly in any order. If they apply to the same column, run the most specific one first — a broad pattern applied first can consume the text the narrower one was looking for.

</details>

<details>
<summary>Will it touch my header row or my other columns?</summary>

Neither, by default. The header row is excluded from replacement unless you tick **Also rewrite the header row**, so a pattern like `code` can rewrite data values without renaming the `code` column. And any column you did not list is copied through byte-for-byte — not re-parsed, not re-typed, not reformatted.

Turn off **First row is a header** for a headerless table; every row then counts as data, and columns have to be given as 1-based indices or ranges since there are no names to match. If a column name you type is not found, the tool stops and lists the available names rather than guessing at the closest one.

</details>

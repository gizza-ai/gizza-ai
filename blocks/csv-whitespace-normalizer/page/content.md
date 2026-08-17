## About this tool

Stray whitespace is the quietest way for a CSV to be wrong. ` Berlin` and `Berlin` are different keys, so a join drops rows, a `VLOOKUP` returns `#N/A`, a `GROUP BY` splits one city into two, and a duplicate check finds nothing to merge — all while the spreadsheet on screen looks perfectly fine, because a space renders as nothing.

A plain trim only fixes half of it. Trimming turns `"  Ada   Lovelace  "` into `"Ada   Lovelace"` and leaves the three spaces in the middle, so the value still does not match `Ada Lovelace`. This tool handles both ends **and** the interior: it collapses runs of whitespace inside a value down to a single space, or removes them outright when the value is a SKU, an IBAN or a part number that should have no spaces at all.

It works on the parsed table, not on the raw text. Fields are read with a real RFC 4180 parse, so quoted values keep their quoting, an embedded comma or newline never breaks a record, the field separator round-trips unchanged, and a ragged row keeps its length. Only whitespace changes — nothing is retyped, reordered, dropped, or invented.

### Worked example

Input (defaults everywhere — the dots below mark the spaces you cannot see):

```text
name,city,sku
··Ada···Lovelace·,·New···York·,··AB·12··CD·
Grace Hopper,Berlin·,XY34
```

Output:

```text
name,city,sku
Ada Lovelace,New York,AB 12 CD
Grace Hopper,Berlin,XY34
```

Row 3 shows what "already clean" looks like: `Grace Hopper` keeps its single space, and only the stray one after `Berlin` goes. If you also wanted `AB 12 CD` to become `AB12CD` — but only in that column — set **Whitespace inside the value** to *Remove* and **Only these columns** to `sku`.

### The invisible ones

By default whitespace means the whole Unicode `White_Space` set, not just space and tab. That matters because the characters a spreadsheet, a PDF, or a web page leaves behind on copy-paste are usually not plain spaces:

| character | code point | where it comes from |
| --- | --- | --- |
| non-breaking space | `U+00A0` | HTML `&nbsp;`, Word, thousands separators in European locales |
| narrow no-break space | `U+202F` | typographic thin spacing, French punctuation |
| ideographic space | `U+3000` | CJK text and CJK-locale spreadsheets |
| en / em space | `U+2002` / `U+2003` | typeset documents |

All of them are trimmed and collapsed like an ordinary space. Switch **What counts as whitespace** to *ASCII only* when a non-breaking space is load-bearing in your data and must survive.

Zero-width characters are deliberately *not* included: `U+200B` (zero-width space) and `U+FEFF` (BOM / zero-width no-break space) have `White_Space=No`, they are not padding, and stripping them is a different job with different consequences.

### Controls

- **Delimiter** — `comma` (default), `tab`, `semicolon`, `pipe`, any single character, or `auto` to sniff the separator from the first line. Whatever comes in goes back out.
- **Trim the cell edges** — `both` ends (default), `leading` only, `trailing` only, or `none` to leave the edges alone and rewrite just the interior. An edge run that survives is copied verbatim, so one-sided trimming really is one-sided.
- **Whitespace inside the value** — `collapse` every run to one plain space (default), `remove` it entirely, or `keep` it, which makes this a pure trim.
- **What counts as whitespace** — `unicode` (default) or `ascii` (space, tab, newline, carriage return, form feed).
- **Only these columns** — comma-separated column names (needs a header), 1-based positions, and inclusive ranges, e.g. `name,city` or `1,3-5`. Blank means every column.
- **First row is a header** — on by default. The header supplies the names used by the column filter.
- **Normalize the header cells too** — on by default, because ` first name ` is exactly the kind of header that loads as a column nobody can reference. Turn it off to copy the header row through byte-for-byte.

### Limits and edge cases

The table is capped at 5,000,000 bytes. A cell that is *only* whitespace becomes empty under every trim setting except `none`. Under `collapse`, a newline embedded inside a quoted cell counts as a whitespace run and becomes one space, which is usually what you want but does flatten a deliberately multi-line cell — choose `keep` to preserve it. A column selector token that parses as a number or a range is read as a position, so a header literally named `3` or `2-4` has to be selected by its position instead of its name. Output rows are terminated with a single newline (`\n`) and the result always ends with one. Everything runs locally in your browser — the table is never uploaded.

## FAQ

<details>
<summary>How is this different from just trimming each cell?</summary>

Trimming only touches the two ends. `"  Ada   Lovelace  "` trims to `"Ada   Lovelace"`, which still will not match `"Ada Lovelace"` in a join or a duplicate check. This tool also rewrites the whitespace *between* the first and last non-whitespace character — collapsing each run to one space, or deleting it. Set **Whitespace inside the value** to *Keep* if a plain trim is genuinely all you want.

</details>

<details>
<summary>My spaces look normal but the tool still changes them — why?</summary>

They are probably not spaces. A copy-paste out of a spreadsheet, a PDF, or a web page routinely brings along `U+00A0` (non-breaking space), `U+202F`, or `U+3000`, all of which render identically to a space and none of which a `TRIM()` or a `strip()` on ASCII whitespace will touch. That is precisely the case this tool is built for. If those characters are meaningful in your data, switch **What counts as whitespace** to *ASCII only*.

</details>

<details>
<summary>Will it break quoted fields, embedded commas, or multi-line cells?</summary>

No. The table is parsed as real CSV before anything is rewritten, so a quoted `"Boston, MA"` stays one field and comes back out quoted, and a cell containing a newline stays one cell. The one thing to know: an embedded newline is whitespace, so under `collapse` it becomes a single space and the cell ends up on one line. Choose `keep` for the interior if you need multi-line cells preserved exactly.

</details>

<details>
<summary>Can I clean only some columns?</summary>

Yes — put names, 1-based positions, or inclusive ranges in **Only these columns**, e.g. `email,city`, `2`, or `1,3-5`. Every other column is copied through untouched, so you can strip every space out of a SKU column without also welding shut the free-text notes next to it. Names need **First row is a header** switched on.

</details>

<details>
<summary>Does it change anything other than whitespace?</summary>

No. Values are not retyped, rounded, re-cased, or re-quoted beyond what the CSV grammar requires; rows are never added, dropped, or reordered; a short row stays short; and the delimiter you fed in is the delimiter you get back. Missing-value tokens, header naming, and duplicate rows are separate jobs with separate tools — this one only moves whitespace.

</details>

<details>
<summary>Does my file get uploaded?</summary>

No. The whole pass is compiled to WebAssembly and runs in the page, so the table never leaves your browser. For a file on disk you can run the same code offline from the terminal: `gizza tool csv-whitespace-normalizer input="$(cat data.csv)"`.

</details>

## About this field extractor

This tool pulls specific columns or character ranges out of every line of text —
think of it as a friendly, browser-local version of the Unix `cut` and `awk`
column tricks, with no files to upload and no command line to remember.

Paste your text, pick **Fields** or **Characters**, and describe what you want
with 1-based selectors:

```
alice 30 engineer
bob 25 designer
carol 41 writer
```

Selecting fields `1,3` with a blank delimiter (whitespace) gives:

```
alice engineer
bob designer
carol writer
```

### Selector syntax

- **Single columns**: `1`, `3` (1-based, counted from the left).
- **Negative indices**: `-1` is the last field, `-2` the second-to-last — no
  need to know how many columns each line has.
- **Ranges**: `2-4` takes fields 2, 3 and 4. `4-2` reverses them.
- **Open-ended ranges**: `3-` takes field 3 through the end (like `cut`).
- **Reordering**: selectors emit in the order you write them, so `3,1,2` moves
  column 3 to the front.
- Endpoints can be negative too: `-3--1` is the last three fields, `-2-` is the
  last two.

### Delimiters

Leave **Delimiter** blank to split on runs of whitespace (tabs and multiple
spaces collapse, like awk). Otherwise type any delimiter: a comma, a pipe `|`, a
multi-character string like `::`, a keyword (`tab`, `comma`, `pipe`,
`semicolon`, `colon`, `space`), or an escape (`\t`, `\n`).

The **output delimiter** controls how the extracted pieces are joined. Blank
reuses the input delimiter; set it to `\t`, a comma, or the keyword `newline` to
put each extracted piece on its own line.

### Character mode

Switch to **Characters** to cut by character position instead of by column —
useful for fixed-width codes and IDs. `1-4` keeps the first four characters of
every line. Character mode counts Unicode code points, so accented letters and
emoji are never split in half.

### Options and limits

- An explicitly numbered single field that is out of range emits an empty
  string (matching `cut`); a range simply stops at the last available field.
- **Trim** removes surrounding whitespace from each extracted field.
- **Skip empty lines** drops blank or whitespace-only lines from the output.
- **Skip header row** ignores the first line before extracting.
- This is a simple splitter, not an RFC-4180 CSV parser: it does not understand
  quoted fields that contain the delimiter. For quoted CSV, use the dedicated
  CSV tools instead.

Everything runs locally in your browser. Your text is never uploaded.

### FAQ

<details>
<summary>How do I get the last column when rows have different widths?</summary>

Use `-1` as the selector. Negative indices count from the end of each line, so
`-1` is always the last field and `-2` the second-to-last, regardless of how
many columns a given row has.

</details>

<details>
<summary>What does a blank delimiter do?</summary>

A blank delimiter splits on runs of whitespace and collapses them, exactly like
awk's default field splitting. So `a    b\tc` becomes three fields. Type a
specific delimiter (comma, `|`, `::`, `\t`, …) to split on that instead.

</details>

<details>
<summary>Can I reorder or reverse columns?</summary>

Yes. Selectors emit in the order you write them, so `3,1,2` puts column 3 first.
A descending range like `4-1` reverses fields 1 through 4.

</details>

<details>
<summary>How is character mode different from field mode?</summary>

Field mode splits each line into columns by a delimiter and selects whole
columns. Character mode ignores delimiters and selects by character position, so
`1-4` keeps the first four characters of every line — handy for fixed-width IDs.
It is Unicode-safe and never splits a multi-byte character.

</details>

<details>
<summary>Does it handle quoted CSV fields?</summary>

No. This is a plain splitter, like `cut -d`, so a comma inside a quoted field is
treated as a separator. For proper RFC-4180 CSV with quoted fields, use the
dedicated CSV column tools.

</details>

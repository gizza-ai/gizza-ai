## About this tool

**CSV transpose** flips a CSV on its diagonal: rows become columns and columns
become rows. The first column of the output is what used to be the header row, so:

```
name,age          name,Ada,Bo
Ada,36     ->     age,36,40
Bo,40
```

Ragged rows are padded with empty cells so the result is rectangular. Transposing
twice gives you the original back. Works with `,` / tab / `;` / `|` delimiters.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat.

### Common uses

- Turn a wide table into a tall one (or vice versa) for a different tool.
- Put each record in a column for side-by-side comparison.

## FAQ

<details>
<summary>What happens when rows have different lengths?</summary>

The output is padded to the widest row: any missing cell becomes an empty cell,
so a 3-column header over a 2-cell data row transposes to three rows where the
third has a blank value. The result is always rectangular.

</details>

<details>
<summary>Does my CSV need a header row?</summary>

No. The transpose is purely positional — whatever is in row 1 simply becomes
column 1 of the output. If you do have a header, it ends up as the first
column, which is usually exactly what you want for side-by-side record
comparison. Transposing the result again returns the original table.

</details>

<details>
<summary>Which delimiters can I use, and are quoted cells safe?</summary>

Comma is the default; `tab`, `semicolon`, and `pipe` are accepted by name, and
any other single character works too. The output is written with the same
delimiter. Cells are parsed as real CSV, so quoted values containing commas or
newlines survive the transpose and are re-quoted where needed.

</details>

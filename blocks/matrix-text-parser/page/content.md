## About this tool

A matrix rarely arrives in the shape you need it. It comes out of a paper as a LaTeX `bmatrix`, out of a notebook as `np.array([[1, 2], [3, 4]])`, out of a colleague's MATLAB session as `[1 2; 3 4]`, out of a spreadsheet as tab-separated rows, and out of a chat message as plain numbers with ragged spacing. Every downstream tool wants one thing: a rectangular 2D array.

This parser normalizes all of those into the same result. It detects which syntax you pasted, splits the rows and cells, types each value, and reports the shape — how many rows, how many columns, whether the matrix is square, and whether every cell is numeric. From there you can re-emit it as JSON, CSV, TSV, MATLAB, NumPy, LaTeX, or column-aligned text.

Detection is automatic but never mandatory. If a paste is ambiguous — a single line containing semicolons could be MATLAB rows or semicolon-delimited columns — pick the input syntax and cell delimiter yourself.

### Worked example

Input (whitespace rows, the default settings):

```text
1 2 3
4 5 6
```

Output:

```json
{
  "shape": [2, 3],
  "rows": 2,
  "columns": 3,
  "square": false,
  "numeric": true,
  "input_format": "delimited",
  "delimiter": "space",
  "matrix": [[1, 2, 3], [4, 5, 6]]
}
```

The same matrix pasted as `np.array([[1, 2], [3, 4]])` with the output set to **MATLAB** returns:

```text
[1 2; 3 4]
```

And a LaTeX environment pasted straight from a paper:

```text
\begin{bmatrix} 1 & 2 \\ 3 & 4 \end{bmatrix}
```

with the output set to **Bare 2D JSON array** and the indent slider at 0 returns:

```json
[[1,2],[3,4]]
```

### Uneven rows and headers

Real pastes are often ragged. With **Uneven row lengths** left at *Fail and name the row*, `1 2 3` followed by `4 5` reports which row broke the rectangle. Switch to *Pad* to extend short rows with the fill value, or *Trim* to cut every row down to the shortest.

If the first line holds column names rather than data, tick **First row is column labels**. The labels come back in the JSON `header` field, ride along as the first CSV/TSV/aligned row, and appear as a comment line above MATLAB, NumPy and LaTeX output, so nothing is silently dropped.

## Options and limits

- Accepted input syntaxes: delimited rows, Python/NumPy nested lists, MATLAB/Octave semicolon rows, and LaTeX `bmatrix`/`pmatrix`/`vmatrix`/`array` environments.
- `np.array(...)`, `np.matrix(...)`, `torch.tensor(...)` and `tf.constant(...)` wrappers are stripped, along with trailing keyword arguments such as `dtype=float`.
- Auto delimiter detection picks the first of tab, comma, semicolon or pipe that appears in the rows, and otherwise splits on whitespace.
- Lines beginning with `#`, `//` or `%` are treated as comments and skipped, as are blank lines and trailing commas left by copy-pasted code.
- Cell values understand plain integers, decimals, scientific notation such as `1.2e-4`, a leading `+` or `-`, and — while **Evaluate fractions** is on — plain fractions such as `3/4`. `NaN` and `Infinity` are kept as text because JSON cannot represent them.
- Arithmetic expressions (`2/3+3*(10-4)`, `2^0.5`) and symbolic constants (`pi`, `e`, `i`) are **not** evaluated. With cell typing on *Auto* they survive verbatim as text cells.
- Limits per run: 2,000,000 bytes, 10,000 rows, 2,000 columns, and 1,000,000 cells.
- The JSON indent slider applies to the JSON and bare-array outputs only; 0 produces minified output.

## FAQ

<details>
<summary>How does auto-detection decide which syntax I pasted?</summary>

A `\begin{...}` token, or cells separated by `&` with rows separated by `\\`, means LaTeX. Brackets containing more brackets — `[[1,2],[3,4]]` or `{{1,3},{4,5}}` — mean a Python or NumPy nested list. Brackets containing semicolons, or a single line containing semicolons, mean MATLAB. Anything else is treated as delimited rows, one row per line. You can always override the guess with the input syntax selector.

</details>

<details>
<summary>What happens to text cells like variable names?</summary>

With cell typing on *Auto* they stay as JSON strings, so a matrix of symbols such as `a b` / `c d` parses fine and the report's `numeric` field comes back `false`. Choose *Numbers only* if you want the parse to fail instead — the error names the exact row and column of the first cell that is not a number. Choose *Keep every cell as text* when values like `007` must not lose their leading zeros.

</details>

<details>
<summary>Why does my single line with semicolons parse as MATLAB rows?</summary>

A lone line such as `1 2; 3 4` is far more often MATLAB row syntax than semicolon-delimited columns, so auto-detection treats it that way. If you really do have one row of semicolon-separated values, set the input syntax to *Delimited rows* and the delimiter to *Semicolon*.

</details>

<details>
<summary>Does it change my numbers?</summary>

No. Values round-trip through a 64-bit float without any rounding or re-formatting step, integers stay integers, and scientific notation is preserved in value. The only conversion is the optional fraction evaluation, which turns `3/4` into `0.75` and can be switched off.

</details>

<details>
<summary>Can it transpose, invert or multiply the matrix?</summary>

No — this tool only normalizes the shape and syntax. It is the step before those operations, and it deliberately stays lossless so the parsed array can be fed into a separate transpose, statistics or linear-algebra step without second-guessing what the parser did to the values.

</details>

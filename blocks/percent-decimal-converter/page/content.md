## About this tool

**Percent Decimal Converter** rewrites a pasted column or delimited table between
percentage-side values and decimal fractions. Use it for spreadsheet columns,
analytics exports, finance rates and configuration files where `12.5%` needs to
become `0.125`, or where a decimal fraction like `0.125` needs to become
`12.5%`.

The conversion is exact for plain decimal input: the tool shifts the decimal
point on the digits you typed instead of multiplying a binary floating-point
number. That keeps `0.1` as exactly `10%`, not `10.000000000000002%`, and keeps
`12.345%` as exactly `0.12345`.

### Worked example

Input:

```csv
rate
12.5%
7%
0.5%
```

With **Direction = Percent side → decimal fraction**, the output is:

```csv
rate
0.125
0.07
0.005
```

Going the other way, `0.125` becomes `12.5%`. Leave **Direction** on
**Auto-detect per cell** when a column is mixed: cells ending in `%`, `‰`, `bp`
or `bps` are converted down to decimal fractions, while bare numbers are
converted up to the selected percent-side unit.

### Options

- **Percent-side unit** controls how far the decimal point moves: percent (`%`)
  moves two places, per mille (`‰`) moves three, and basis points (`bps`) moves
  four.
- **Columns to convert** can be blank for every column, a comma-separated list of
  one-based indices such as `2,4`, or header names such as `rate,share` when the
  header option is on.
- **First row is a header** keeps row 1 unchanged and enables header-name column
  selection. Turn it off for a headerless list.
- **Delimiter** accepts `comma`, `tab`, `semicolon`, `pipe` or any single
  character. The same delimiter is used for output.
- **Decimal places** from `0` to `12` rounds and pads to a fixed width. `-1` is
  exact mode with no rounding or padding.
- **Trim trailing zeros** turns a fixed width into an “at most” width after
  rounding, so `50%` with `decimals=4` can become `0.5` instead of `0.5000`.
- **Append suffix** adds `%`, `‰` or ` bps` when converting up to the
  percent-side unit. Turn it off when the column heading already says percent.

### Limits and edge cases

- Input is limited to **5 MB**.
- Plain decimal numbers are converted. Text, blank cells, currency values and
  scientific notation are copied through unchanged rather than guessed.
- Thousands separators in quoted cells are tolerated, for example
  `"1,234.5%"`.
- The tool preserves row order, selected/unselected columns and the chosen
  delimiter. CSV quoting may be normalized by the CSV writer when fields need it.
- Comma decimal notation such as `12,5%` is not parsed as twelve point five; use
  a dot decimal (`12.5%`) and choose a semicolon delimiter for European-style
  CSV exports.

Everything runs locally in WebAssembly in your browser; your table is not
uploaded.

## FAQ

<details>
<summary>What is the difference between a percentage and a decimal fraction?</summary>

A percentage is a value per hundred. To convert a percentage to a decimal fraction, move the decimal point two places left: `12.5%` becomes `0.125`. To convert a decimal fraction to a percentage, move it two places right: `0.125` becomes `12.5%`.

</details>

<details>
<summary>When should I use auto-detect?</summary>

Use **Auto-detect per cell** for mixed pasted data. A cell with `%`, `‰`, `bp` or `bps` is treated as already being on the percent side and is converted down to a fraction. A bare number is treated as a fraction and converted up to the selected unit.

</details>

<details>
<summary>Can I convert only one column in a CSV?</summary>

Yes. Leave **First row is a header** on and enter a header name such as `rate`, or use one-based indices such as `2` or `2,4`. Blank **Columns to convert** means every numeric cell in every column is eligible for conversion.

</details>

<details>
<summary>How do basis points work?</summary>

A basis point is one hundredth of one percent. The basis-points unit moves the decimal point four places between the raw fraction and the displayed value: `0.0025` becomes `25 bps`, and `25 bps` becomes `0.0025`.

</details>

<details>
<summary>Why are some cells left unchanged?</summary>

Only plain decimal numbers are converted. Text labels, blanks, currency strings such as `$12.99`, and scientific notation such as `1e5` are copied through unchanged so the tool does not silently corrupt cells it cannot interpret safely.

</details>

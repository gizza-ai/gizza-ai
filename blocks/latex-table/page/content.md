## What this tool does

Paste **CSV** or **TSV** data and get a ready-to-paste **LaTeX `tabular`** back,
right in your browser. Nothing is uploaded — it runs locally, works offline, and
needs no sign-up. Pick a **Style**, set the **Alignment**, and optionally add a
**Caption** / **Label** to wrap it in a `table` float.

## Styles

| Style | What you get | Notes |
| --- | --- | --- |
| **booktabs** (default) | `\toprule` · `\midrule` · `\bottomrule` | The clean, publication-quality look. Needs `\usepackage{booktabs}`. |
| **grid** | Vertical bars `|l|c|r|` and a `\hline` between every row | The classic boxed look. |
| **plain** | No rules at all | Just the rows. |

## Options

- **Delimiter** — `auto` detects CSV vs TSV from the first line, or force it with
  `comma`, `tab`, `;`, `|`, or any single character.
- **Alignment** — a single `l`, `c`, or `r` applied to every column, or a
  per-column string like `lcr` (must match the number of columns).
- **First row is a header** — on by default; the header is split from the body by
  a `\midrule` (booktabs) or `\hline` (grid).
- **Escape LaTeX special characters** — on by default; turns `& % $ # _ { } ~ ^ \`
  into their escaped forms so the table compiles. Turn it off to keep raw LaTeX
  (e.g. `$x^2$` in a cell).
- **Bold header cells** — wrap each header cell in `\textbf{…}`.
- **Caption / Label** — set either one to wrap the `tabular` in a centered
  `\begin{table}[ht]` float with `\caption{…}` and `\label{…}` for cross-references.

## Example

Input (CSV):

```
City,Population,Area
Tokyo,37400068,2194
Delhi,32900000,1484
```

Output (`booktabs`, alignment `lrr`):

```latex
\begin{tabular}{lrr}
\toprule
City & Population & Area \\
\midrule
Tokyo & 37400068 & 2194 \\
Delhi & 32900000 & 1484 \\
\bottomrule
\end{tabular}
```

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your data never leaves your device, and the tool
keeps working offline once the page has loaded.

</details>

<details>
<summary>Do I need any LaTeX packages?</summary>

Only for `booktabs` style: add
`\usepackage{booktabs}` to your preamble. `grid` and `plain` need nothing extra.

</details>

<details>
<summary>My cells contain <code>%</code> or <code>&amp;</code> — will it break?</summary>

No. Escaping is on by default, so
special characters are converted to their LaTeX-safe forms. Turn escaping off only
if you want to keep raw LaTeX inside a cell.

</details>

<details>
<summary>Can I align columns differently?</summary>

Yes — put a per-column string like `lcr` in the
Alignment field (one letter per column), or a single letter to align them all the same.

</details>

<details>
<summary>Ragged rows?</summary>

Short rows are padded with empty cells so every row has the same
number of columns.

</details>

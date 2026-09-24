# matrix-text-parser — competitor analysis (2026-09-24)

Scan run **before** implementing, per the create-next-tool recipe. All findings are
paraphrased from public product pages; no competitor copy, branding, or trademark text is
reproduced or reused anywhere in this block.

Backlog row (tools-to-build.csv:1758): *"Parses a matrix pasted in any common form
(space/comma/tab rows, Python/NumPy list, semicolon MATLAB syntax) into a normalized 2D JSON
array with shape."* — type hint `pure`.

## Duplicate check (done before the scan)

`ls blocks/ | grep -i -E 'matri|vector|linear|transpose|numpy'` plus targeted greps:

| Existing block | Why it is not this tool |
| --- | --- |
| `ndjson-to-matrix` | Takes NDJSON **records**, flattens dotted paths and unions columns into a grid. Its input is JSON Lines, never a pasted MATLAB/NumPy/whitespace matrix literal. |
| `adjacency-matrix-converter` | Graph-specific: adjacency matrix ⇄ edge list. Assumes a labelled square graph matrix, not arbitrary pasted numeric text. |
| `confusion-matrix` | Classification metrics from label pairs; the "matrix" is computed, not parsed. |
| `document-term-matrix` | Builds a term matrix from a text corpus. |
| `csv-transpose`, `text-to-table`, `csv-to-table` | CSV/delimited text only — no bracketed, semicolon, LaTeX or NumPy syntax, and no shape/normalization report. |
| `npy-array-decoder`, `wav-to-numpy-npy` | Binary `.npy` container, not pasted text. |
| `risk-matrix`, `vectorize`, `vector-similarity` | Unrelated despite the names. |

A grep for `matlab`/`semicolon.*matrix`/`numpy literal` across every `blocks/*/core/src/lib.rs`
returns exactly one unrelated hit (`correlated-sample-generator`). No block parses bracketed
matrix syntax. **Not a duplicate — proceed.**

## Competitors reviewed

### 1. matrixcalc.org — matrix calculator with a free-form literal input

- Accepts a brace literal such as `{{1,3},{4,5}}`, with commas **or** semicolons as separators.
- Cells accept far more than plain decimals: fractions (`1/3`), scientific notation (`1.2e-4`),
  repeating-decimal notation, and full arithmetic expressions (`2/3+3*(10-4)`, `2^0.5`), plus
  symbolic constants (`pi`, `e`, `i`) and identity-matrix shorthand.
- Grid editor with keyboard navigation and clipboard/drag-drop transfer from a text editor —
  i.e. pasting a matrix from elsewhere is the expected entry path.
- Ragged input is tolerated by leaving extra cells empty (non-square matrices supported).
- Display precision is user-controlled (fraction digits / significant digits).
- No explicit size cap documented.

### 2. onlinetools.com — math matrix utilities (transpose / format family)

- Separate **element separator** and **row separator** controls, on the input *and* the output
  side, defaulting to space and newline respectively.
- A "prettify" toggle that column-aligns every element in the result.
- Four one-click preset examples (3×3, 8×8, alphabetic-variable cells, non-square) that also
  set the option values — presets are the primary discovery mechanism.
- Explicitly supports **non-numeric cells** (variables like `a`, `b`) as first-class values.
- Cross-links to a family of sibling matrix tools (invert, determinant, add, multiply,
  identity, random).

### 3. tableconvert.com — table/CSV ⇄ JSON converter

- Output-shape selector including an explicit **2D array** mode alongside array-of-objects,
  column-array and keyed-array.
- **Delimiter auto-detection** across comma/tab/semicolon/pipe with no manual step.
- First-row-as-header toggle that promotes row 1 to keys.
- Minify toggle + indent size (2 / 4 / 8 / tab).
- Large source-format menu (Markdown, LaTeX, HTML, MediaWiki, SQL, XML, Excel), so multi-syntax
  input and multi-syntax output are both treated as table stakes in this category.

## Table-stakes → decisions

Every table stake below lands in the descriptor or in the "considered, not built" list; none
was dropped silently.

| # | Table stake (source) | Fit | Where it lands |
| --- | --- | --- | --- |
| 1 | Bracketed literal input, comma **or** semicolon separated (1) | in-model | `input_format` = `python` / `matlab`, auto-detected |
| 2 | Plain delimited rows, space/comma/tab (backlog row, 2, 3) | in-model | `input_format = delimited` + `delimiter` |
| 3 | Delimiter auto-detection (3) | in-model | `delimiter = auto` default; detected value reported in the JSON output |
| 4 | Explicit element-separator override (2, 3) | in-model | `delimiter` = auto/comma/space/tab/semicolon/pipe |
| 5 | Fractions like `1/3` as cell values (1) | in-model | `fractions` boolean, default on |
| 6 | Scientific notation `1.2e-4` (1) | in-model | core numeric parse (always on) |
| 7 | Non-numeric / symbolic cells (2) | in-model | `cells = auto` keeps them as JSON strings; `cells = number` rejects them with a row/column-located error |
| 8 | 2D-array JSON output (3, backlog row) | in-model | `output = json` (array + shape metadata) and `output = array` (bare array) |
| 9 | Minify + indent width (3) | in-model | `indent` 0–8 slider; 0 = minified |
| 10 | Shape / dimensions report (backlog row) | in-model | `shape`, `rows`, `columns`, `square`, `numeric` in the JSON output |
| 11 | Ragged rows tolerated rather than rejected (1) | in-model | `ragged` = `error` / `pad` / `trim`, with `fill` |
| 12 | First row as header (3) | in-model | `header` boolean → labels returned separately as `header` |
| 13 | Column-aligned "prettified" rendering (2) | in-model | `output = aligned` |
| 14 | Re-emit in another syntax (2, 3) | in-model | `output` = `csv` / `tsv` / `matlab` / `numpy` / `latex` |
| 15 | LaTeX matrix environments as **input** (3 treats LaTeX as a first-class source) | in-model | `input_format = latex`, auto-detected from `\begin{…matrix}` / `\begin{array}` |
| 16 | One-click preset examples (2, 3) | in-model | four `[[example]]` chips on the page |
| 17 | Comment / blank-line tolerance in pasted dumps | in-model | `#`, `//` and `%` line comments skipped; blank lines ignored |
| 18 | Arithmetic-expression cells `2/3+3*(10-4)`, `2^0.5`, `sin(phi)` (1) | **out-of-model** | A general expression evaluator is a different tool; `blocks/calculator` already owns it, and silently evaluating pasted cells would be surprising in a *parser*. Only the plain `a/b` fraction form is evaluated. |
| 19 | Symbolic constants `pi`, `e`, `i`, complex entries (1) | **out-of-model** | Same reason as 18. With `cells = auto` they survive verbatim as strings, so nothing is lost — they are just not evaluated. |
| 20 | Identity/random matrix generation, invert/determinant/multiply (1, 2) | **out-of-model** | Generation and linear algebra are separate tools, not parsing. Noted as future backlog siblings. |
| 21 | Transpose (2) | **considered, rejected** | Already shipped as `blocks/csv-transpose`; adding a flag here would split the same capability across two blocks. |
| 22 | Grid/spreadsheet cell editor with keyboard navigation (1) | **out-of-model** | Needs a bespoke stateful grid widget; the generator is a declarative form renderer, and a paste-first tool does not need cell-by-cell entry. |
| 23 | File upload of a matrix (2) | **out-of-model** | This repo has no generic pure-wasm binary/file page input (file inputs exist only for `runtime = "ffmpeg"`/`"model"`). Paste covers the use case. |
| 24 | Per-cell display precision / rounding (1) | **considered, rejected** | The tool's contract is *normalize without changing values*; rounding on output would make the parser lossy. Scientific notation is preserved through `f64` round-tripping. |

## Resulting descriptor

`matrix` (required, multiline) · `input_format` (auto|delimited|python|matlab|latex) ·
`delimiter` (auto|comma|space|tab|semicolon|pipe) · `output`
(json|array|csv|tsv|matlab|numpy|latex|aligned) · `cells` (auto|number|string) ·
`fractions` (bool, default true) · `header` (bool, default false) · `ragged`
(error|pad|trim) · `fill` (string, default `0`) · `indent` (0–8, default 2).

Limits stated on the page and enforced in the core: 2,000,000 input bytes, 10,000 rows,
2,000 columns, 1,000,000 cells.

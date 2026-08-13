# absolute-value-transformer — competitor analysis (2026-08-13)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased
observations of what each tool *does*; no competitor copy, branding, or trademark text is reused.

## Dup check (why this tool is not a duplicate)

| Existing block | What it does | Why it is not this tool |
| --- | --- | --- |
| `column-math` | element-wise add/subtract/multiply/divide between **two** equal-length columns | binary, needs a second column; has no unary abs/sign/negate mode |
| `csv-formula-eval` | general `col = expr` formula engine over a CSV table (supports an `abs` function) | requires CSV with a header row and formula syntax; not a one-click unary transform of a pasted number column |
| `numeric-string-sanitizer` | strips currency/separators/units/percents from *messy text* cells | cleans notation, never changes a value's sign or magnitude |
| `numeric-range-check` | flags CSV cells outside a min/max range | validation/reporting, emits no transformed column |

Confirmed by reading each block's `core/src/lib.rs` / `src/lib.rs` descriptor. No block performs
absolute value, signum extraction, or sign flipping on a numeric column.

## Competitors reviewed

1. **Online Tools — number-list utilities** (`onlinetools.com/number/*`, e.g. the maximum-number
   tool; the number family is the closest structural match) — paste a number list, pick the **input
   separator** (line break by default, or comma/semicolon/custom), pick an **output separator**,
   toggle an "absolute" (modulus) option, then copy/download/export the result. Accepts integers,
   decimals, and fractions. Import-from-file is offered alongside the textarea.
2. **CalculatorSoup — absolute value calculator** — single-value only (`|x|`), non-negative output,
   with an explanation of the number-line definition and three worked cases (9 → 9, −9 → 9, 0 → 0).
3. **TrumpExcel — change negative to positive** — a methods guide, but it defines the spreadsheet
   user's real requirements: `ABS()`, multiply-by-−1 to flip, Paste-Special multiply, Flash Fill,
   VBA. Explicitly calls out **helper column vs. in-place**, and that `ABS()` returns `#VALUE!` on
   text cells while the `IF` variant silently ignores them.
4. **Exceljet — change negative numbers to positive** — `=ABS(number)` as the canonical formula,
   plus the Paste-Special ×(−1) one-shot; also documents the adjacent `MAX(x,0)` clamp.
5. **MedCalc / vCalc — SIGN (signum) function** — the sign-extraction half of the backlog row:
   −1 for negative, 0 for zero, +1 for positive, and the identity `x = sign(x) · |x|`.

## Table stakes → decisions

| Capability | Seen in | Verdict |
| --- | --- | --- |
| Absolute value over a whole pasted column | 1, 2, 3, 4 | **in-model** — `operation = abs` (default) |
| Sign / signum extraction (−1 / 0 / 1) | 5 | **in-model** — `operation = sign` |
| Flip signs (× −1) | 1, 3, 4 | **in-model** — `operation = negate` |
| Force all values negative (accounting "make it a debit") | 3 | **in-model** — `operation = force-negative` (−\|x\|) |
| Configurable **input** separator, line break by default | 1 | **in-model** — `separator = auto\|newline\|comma\|space\|semicolon\|tab\|pipe` (`auto` detects) |
| Configurable **output** separator | 1 | **in-model** — `output_separator`, defaults to mirroring the input |
| Keep the original alongside the result (helper column) | 3 | **in-model** — `output = table` emits `original<TAB>result`; `output = json` for structured use |
| Explicit non-numeric handling (`#VALUE!` vs ignore) | 3 | **in-model** — `on_error = fail\|skip\|keep\|blank` |
| Rounding the transformed values | 1 (decimal handling) | **in-model** — `decimals = auto\|0…6` |
| Summary stats over the result | 1 (sum/min/max family) | **in-model** — `stats` checkbox (count/sum/min/max/mean) |
| Worked examples + number-line explanation on the page | 2, 5 | **in-model** — `content.md` worked example + FAQ |
| Preset one-click modes | 1 (option toggles) | **in-model** — `[[example]]` chips for each operation |
| Import from file / drag-drop upload | 1 | **out-of-model** — pure text-in/text-out block; the page has no file input for pure tools (the generator's `source = "file"` input is ffmpeg-runtime only). Users paste or deep-link. |
| Copy-to-clipboard / download / pastebin export | 1 | **partly platform** — the generator already renders a Download link for `format = "text"` pages; clipboard/pastebin export is site-repo chrome, not a block capability. |
| Fraction input (`1/4`, `-9/2`) | 1 | **out-of-model** — deliberately not accepted; this repo's numeric columns are decimal/scientific. Documented as a limit on the page and rejected with a clear per-token error rather than silently mis-parsed. |
| Currency symbols / thousands separators / `(250.00)` accounting negatives | 3 (implied by messy sheets) | **out-of-model here** — already owned by `numeric-string-sanitizer`; the FAQ points at it instead of duplicating it. |
| Live graph of the transform | 5 | **out-of-model** — plotting is `csv-chart-generator`'s job. |

## Notes

- Nothing above was copied verbatim; every descriptor `.describe()`, page heading, and FAQ answer
  is written fresh for this block.
- Out-of-model items are listed here and (where user-visible) stated as limits on the page; none are
  silently dropped.

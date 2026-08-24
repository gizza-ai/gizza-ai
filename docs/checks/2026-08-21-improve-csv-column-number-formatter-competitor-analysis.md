# csv-column-number-formatter — competitor scan + build decisions (2026-08-21)

Scan run **before** implementing, per `/create-next-tool` step 4. Everything below is a
**paraphrase** of observed behaviour — no competitor copy, branding or trademarks were reused,
and no competitor asset was copied. The purpose of the scan is to fix the table-stakes parameter
set, the defaults, and the UX control patterns for our own original implementation.

Search used: *"format numbers in CSV column online tool decimal places thousands separator"* plus a
follow-up for CSV-specific rounding tools.

## Competitors reviewed

### 1. Absolutool — Number Formatter (browser, single value)

- **Format-type dropdown** with six presets: comma grouping, space grouping, compact
  abbreviation (`1.23M`), Indian grouping (`12,34,567`), scientific (`1.23e+6`), and plain.
- **Decimals** as a free numeric field.
- **Prefix** field, used mostly for currency symbols.
- Locale list (en-US, de-DE, fr-FR, en-IN, ar-EG, ja-JP, zh-CN …) driving separators.
- Rounding modes exposed/discussed: half-expand (its default), half-even (banker's), truncate,
  plus ceil / floor / half-ceil / half-floor / half-trunc.
- Accepts integers, decimals, large values and scientific notation.
- UX: single text field, live preview, Copy and Clear buttons, dropdown + two small fields.
- FAQ covers US-vs-EU conventions, currency, rounding modes, floating-point pitfalls, the Indian
  grouping system, and safe-integer limits.
- **Gap for us:** it formats ONE value. There is no table, no column selection, no CSV
  round-trip — pasting a column into it and getting the rest of the row back is not possible.

### 2. Online Tools — Round a Number (browser, single value)

- **Rounding-mode radio set:** round up, round to nearest (5-and-above goes up), round toward the
  larger value, and round-to-a-half (nearest 0 or 5).
- **Precision field that accepts NEGATIVE values:** `1` = tenths, `2` = hundredths, `3` =
  thousandths, `0` = ones, `-1` = tens, `-2` = hundreds. This is the most useful idea in the whole
  scan and is missing from most "decimal places" fields.
- UX: option panel beside a live output pane, no header/column concept at all.
- **Gap for us:** rounding only. No grouping, no sign styling, no prefix/suffix, no CSV.

### 3. EmEditor — "round numbers in a CSV column to 2 decimal places" (desktop workflow)

- The nearest thing to a real *column* competitor, and it is a **manual multi-step workflow**:
  select the whole column, open Replace, enable the "number range" option (which seeds a numeric
  find pattern), and type a JavaScript expression such as a `parseFloat(...).toFixed(2)` call into
  the replace field with "in the selection only" ticked.
- Documented caveats: the docs do not say what happens to the header row, and the approach assumes
  every selected cell is numeric — a non-numeric cell is simply whatever the expression returns.
- **Gap for us:** requires a paid desktop editor and hand-written JavaScript; no grouping, no
  sign/accounting styles, no audit of what changed, and non-numeric cells are a silent hazard.

## Table stakes extracted

| Table stake | Source | Our decision |
| --- | --- | --- |
| Fixed decimal places | all three | `decimals`, default `2` |
| Negative precision (round to tens/hundreds) | Online Tools | `decimals` accepts `-9 … 15` |
| Multiple rounding modes | Absolutool, Online Tools | `rounding` enum, 6 modes, default `half_up` |
| Thousands grouping | Absolutool | `grouping` enum: `none` / `thousands` / `indian` |
| Choice of group + decimal mark | Absolutool (via locales) | `group_separator`, `decimal_separator` enums |
| Compact / scientific / percent notation | Absolutool | `notation` enum, 4 values |
| Prefix (currency) | Absolutool | `prefix` **and** `suffix` (units like ` kg`, ` %`) |
| Reads messy input (`1.234,56`, `$1,234`, `(250)`) | Absolutool | `input_decimal` enum + tolerant parser |
| Column targeting | EmEditor (manual) | `columns`: names, 1-based indices, `2-4` ranges |
| Header protection | EmEditor gap | `has_header`, header never reformatted |
| Non-numeric cell policy | EmEditor gap | `non_numeric` enum: `keep` / `blank` / `error` |
| Live preview + copy | Absolutool | shared page runtime already provides both |
| Preset chips | Absolutool's format-type dropdown | `[[example]]` chips on the page |

## Decisions that DIFFER from the competitors (and why)

- **`grouping` defaults to `none`, not commas.** Every single-value competitor defaults to comma
  grouping because its output is meant to be read, not re-imported. Ours writes a **CSV**, and a
  grouped value has to be quoted to survive the round-trip and stops parsing as a number in the
  next tool. Machine-safe by default; grouping is one dropdown away and the page says so.
- **No locale dropdown.** A locale list is a *bundle* of the four things we already expose
  (grouping style, group mark, decimal mark, digit ordering). Exposing the primitives means
  `1.234,56` (de-DE), `12,34,567.00` (en-IN) and `1 234,56` (fr-FR) are all reachable without
  shipping — or drifting against — a CLDR table in the wasm bundle.
- **Rounding happens on the DIGIT STRING, not on `f64`.** `1.005` at two places must be `1.01`;
  the naive `x * 100.0` round returns `1.00` because `1.005` is not representable in binary. Same
  correctness stance the sibling `number-to-currency-formatter` block took.
- **`sign` includes `parens` and `space`.** Accounting parentheses and the align-a-column leading
  space are real finance-export needs that none of the three scanned tools offer together.
- **`output = report`** (per-column `cells_formatted,cells_unchanged,non_numeric` audit) has no
  competitor equivalent. It is the "check the rule before you trust it on a real file" step that
  the EmEditor workflow forces you to do by eye.

## Considered, NOT built (out of model or deliberately rejected)

- **Locale preset dropdown (en-IN, de-DE, …)** — rejected: it would bake a partial CLDR table into
  the block and drift. The `[[example]]` chips cover the common shapes instead.
- **Currency-code awareness (`USD` → `$`)** — out of scope here; `blocks/number-to-currency-formatter`
  already owns symbol/code lookup for single values, and `prefix` covers the CSV case.
- **Per-column different formats in one pass** — rejected: one uniform format per run is the whole
  point of the tool ("applies a **uniform** numeric format"); chain two runs for two formats.
- **Cloud/batch file processing, accounts, saved profiles** — out of model (browser-local, wasm,
  no server, no account).
- **Full ICU/CLDR `Intl.NumberFormat` parity (ordinals, currency display names, notation="compact"
  long form such as "1.2 million")** — out of model for a wasm block of this size.

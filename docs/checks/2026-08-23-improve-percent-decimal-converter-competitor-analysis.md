# percent-decimal-converter — competitor analysis (2026-08-23)

Scan run **before** implementation, per `/create-next-tool` step 3 / `/improve-tool` Phase 2.
All findings are paraphrased observations of publicly visible behaviour. **No competitor copy,
branding, or trademark is reproduced anywhere in this block.**

## Scope check — is this a duplicate?

Nearest existing blocks were grepped before building:

| Block | Overlap | Verdict |
|---|---|---|
| `csv-column-number-formatter` | Has a `percent` output *notation* (`0.452` → `45.2%`). But its parser explicitly treats a trailing `%` as **presentation only** — `45.2%` parses as `45.2`, not `0.452` (see its `lib.rs` comment "A trailing percent sign is only presentation on input"). So it covers the decimal→percent direction as a formatting side-effect and **cannot do percent→decimal at all**. | Not a dup; the primary direction is missing. |
| `round-decimals` | Rounds numeric CSV columns. No percent semantics (no ×/÷ 100). | Not a dup. |
| `percentage-calculator`, `percent-difference-calculator`, `cumulative-percent-builder` | Single-value / derived-statistic calculators, not a column transform. | Not a dup. |
| `unit-converter` | Physical units of measure only; no percent/permille/bps. | Not a dup. |

Decision: **build**, with the bidirectional (and auto-detecting) column transform as the core
value, since that is the part no existing block provides.

## Competitors reviewed

1. **CalculatorSoup — percent to decimal** (calculatorsoup.com)
2. **GIGAcalculator — percent to decimal** + its sibling **decimal to percent** (gigacalculator.com)
3. **Calculatio — percent as a decimal** (calculat.io)
4. **Vedantu — percent to decimal** (vedantu.com) — surfaced in search, same single-value shape
5. **Basis-point converter family** (Omni Calculator, basispointcalculator.com, calcipedia.org) —
   the adjacent "one value, many units" shape

## What the competitors actually ship

Every mainstream percent↔decimal tool is a **single-value calculator**: one number in, one number
out, plus a static reference table (1 %→0.01 … 100 %→1.00) and a formula restatement
(`percent ÷ 100 = decimal`, `decimal × 100 = percent`).

Observed table stakes:

| # | Capability | Seen on | Fit | Where it landed |
|---|---|---|---|---|
| 1 | Percent → decimal (÷ 100) | all | in-model | `direction = to_decimal` |
| 2 | Decimal → percent (× 100) | GIGAcalculator, CalculatorSoup (sibling page) | in-model | `direction = to_percent` |
| 3 | Both directions in **one** tool | none of the single-value tools — they split it across two pages | in-model | single `direction` param incl. `auto` |
| 4 | Values > 100 % (`150 %` → `1.5`) | Calculatio | in-model | no cap on magnitude; tested |
| 5 | Negative values (± toggle) | CalculatorSoup | in-model | sign preserved; `-12.5%` → `-0.125` |
| 6 | Fractional percents (`3.25 %`, `33.33 %`) | CalculatorSoup, Calculatio | in-model | exact decimal-string shift, tested |
| 7 | Per-mille (‰) and basis points (bps) as sibling units | basis-point converter family | in-model | `unit = percent\|permille\|basis_points` (÷1e2 / 1e3 / 1e4) |
| 8 | Worked examples / conversion table on the page | all | in-model (copy) | `content.md` conversion table + worked examples, written from scratch |
| 9 | Preset buttons / one-click examples | Omni, basispointcalculator | in-model (UX) | four `[[example]]` chips on the page |
| 10 | Result shown with the unit symbol appended | all decimal→percent tools | in-model | `suffix` boolean (default on) |
| 11 | Precision / rounding control | **none** offer it (all silently emit full float precision) | in-model | `decimals` (−1 = exact, 0–12 = fixed + padded) — a real gap we close |
| 12 | Step-by-step "here's the formula" walkthrough | CalculatorSoup, Vedantu | out-of-model for the *output* | the formula lives in page copy, not in the tool output (a column transform can't narrate per row) |
| 13 | Live currency/amount impact ("x bps of $1 M") | basis-point family | out-of-model | out of scope — that is a calculator, not a column converter; `percentage-calculator` already covers share-of-total maths |
| 14 | Batch / column operation over CSV | **none** — every competitor is one value at a time | in-model | this is our differentiator: `data` + `columns` + `header` + `delimiter` |

## Gaps we close that competitors do not

- **Column/CSV batch.** No reviewed competitor converts a whole column; the workaround people
  post about is a spreadsheet formula or a Power Query custom column. This block does the whole
  file in one run, leaving non-numeric cells and unselected columns untouched.
- **`auto` direction.** Per-cell detection: a cell carrying `%`/`‰`/`bp`/`bps` converts *down* to a
  decimal fraction; a bare number converts *up* to the percent-side unit. Mixed columns work.
- **Exact decimal arithmetic.** The conversion shifts the decimal point on the digit string rather
  than multiplying an `f64`, so `0.1` → `10%` (not `10.000000000000002%`) and `12.345%` → `0.12345`
  exactly. Competitors that use naive float maths show this artefact for some inputs.
- **Rounding control** (`decimals`) with zero-padding — item 11 above, absent everywhere.
- **Input tolerance:** surrounding whitespace, NBSP, thousands separators (`1 234,5` style is out;
  `1,234.5` inside a quoted CSV field is in), a space before the unit (`12.5 %`), and `bp`/`bps`
  in either case.

## Explicitly NOT built (out-of-model / out-of-scope)

- Step-by-step formula narration per value (item 12) — page copy covers the maths instead.
- Currency-impact calculations from basis points (item 13).
- Locale-specific decimal commas (`0,125`) — ambiguous inside comma-delimited CSV; the `delimiter`
  param lets a `;`-separated European file through, but the decimal mark stays `.`.

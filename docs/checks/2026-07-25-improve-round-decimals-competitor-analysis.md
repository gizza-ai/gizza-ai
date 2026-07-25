# round-decimals — competitor analysis (2026-07-25)

Function: round numeric columns of a CSV/number list to a chosen number of decimal
places using a selectable rounding mode. All findings paraphrased — no competitor
copy/branding reproduced.

## Competitors scanned

1. **dCode — Rounding Calculator** (dcode.fr/rounding-calculator)
   - Modes: nearest (classical), floor (down), ceiling (up), "both", round-to-multiple,
     truncate.
   - Precision: round to integer, presets 1/2/3, or custom N decimals; or a multiple.
   - Batch: "Round each value" processes a list; export to .csv/.txt.
   - Default: classical rounding (≥5 rounds up). Example: 3.15 → 3.2 at 1 place.

2. **Flipper File — Round Numbers Online** (flipperfile.com/text-tools/round-numbers-online)
   - Modes: Nearest (standard), Always Up (ceil), Always Down (floor), Truncate.
   - Decimals: quick buttons 0–3, custom up to 10.
   - Toggles: skip whole numbers, skip negatives, skip values under 1, round currency
     symbols, round percentages, preserve comma/thousands formatting.
   - Input: paste / .txt / .docx / drag-drop. Output: copy or .txt download.
   - Reports counts of numbers found / rounded / skipped; "show changes" panel.

3. **CalculatorSoup — Rounding Numbers Calculator**
   - Primary method: Round Half Away From Zero (2.5→3, -2.5→-3). Points to a separate
     "rounding methods" calc for the rest.
   - Positions: ones … billions and tenths … billionths (up to 9 decimal places).
   - Example: 3266.528 → 3266.53 (hundredths).

4. **Pandas `DataFrame.round`** (reference for the column/spreadsheet framing)
   - Rounds all columns to N, or per-column via a dict `{"A":1,"B":2}`.
   - Default mode: round half to **even** (banker's): 0.5→0, 1.5→2, 2.5→2.
   - Non-numeric columns are left untouched.

5. **Excel/Sheets family (ROUND / ROUNDUP / ROUNDDOWN / TRUNC / MROUND)** — the mental
   model most CSV users bring: round-half-away-from-zero by default, plus explicit
   up/down/truncate; negative decimals round left of the point (out of scope here).

## Table-stakes → our decisions

| Capability | Competitors | Decision |
| ---------- | ----------- | -------- |
| Choose N decimal places (0–10) | all | **In** — `decimals`, default 2, min 0, max 10 |
| Nearest / half-away-from-zero | all (default) | **In** — `mode=half_up` (default) |
| Ceiling (always up) | dCode, Flipper | **In** — `mode=ceil` |
| Floor (always down) | dCode, Flipper | **In** — `mode=floor` |
| Truncate (drop digits) | dCode, Flipper | **In** — `mode=truncate` |
| Banker's / half-to-even | Pandas default | **In** — `mode=half_even` |
| Half toward zero | (completeness) | **In** — `mode=half_down` |
| Round-to-multiple | dCode | **Rejected** — different tool (snap to step ≠ decimal places); would confuse the decimals axis |
| Per-column selection | Pandas dict | **In** — `columns` (names or 1-based indices; empty = all numeric columns) |
| Header awareness / delimiter | CSV framing | **In** — `header`, `delimiter` (char or comma/tab/semicolon/pipe) |
| Fixed formatting / pad trailing zeros | "preserve formatting" | **In** — `trailing_zeros` (3 → 3.00) |
| Leave non-numeric cells alone | Pandas | **In** — inherent; non-numeric cells pass through unchanged |
| Skip whole numbers / negatives / <1 | Flipper | **Rejected** — column selection is the CSV-native way to scope which values change; these text-oriented toggles add schema bloat |
| Preserve currency/percent/thousands separators | Flipper | **Out-of-model note** — in a CSV, `,` is the delimiter and `$`/`%` make a cell non-numeric, so such cells pass through unchanged; documented as a limit |
| Export .csv/.txt, drag-drop .docx | dCode, Flipper | **Out-of-model** — page already offers copy + download; .docx/file upload needs a backend/parser, not built |
| Float-precision correctness (1.005→1.01) | most tools get this wrong | **In (differentiator)** — we round on the decimal STRING, not the binary f64, so `1.005` at 2 places is `1.01` as a human expects |

## Notes
- The float-representation trap (`1.005 * 100 = 100.4999…`) makes naive tools return
  `1.00`. We parse the digit string directly for plain decimals and only fall back to
  f64 rounding for scientific-notation cells, which we document.
- Scientific-notation cells (`1e5`) are rounded via the f64 fallback; genuinely
  non-numeric cells (currency symbols, text) are left unchanged.

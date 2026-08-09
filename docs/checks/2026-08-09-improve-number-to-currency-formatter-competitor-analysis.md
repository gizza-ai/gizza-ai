# number-to-currency-formatter — competitor analysis (2026-08-09)

Scan run before finishing implementation so the descriptor and page controls match
what people expect from currency/number formatting utilities. Notes are
paraphrased; no competitor wording, branding, or trademarks are copied into the
tool.

## Competitors reviewed

| # | Tool shape | What it exposes |
|---|------------|-----------------|
| 1 | Browser currency formatter snippets / calculator pages | Number input, currency code/symbol, decimal-place count, thousands separator, decimal separator, and examples such as USD/EUR output. Most are single-result text utilities, not exchange-rate tools. |
| 2 | Intl.NumberFormat demos | Locale and currency code selectors, currencyDisplay choices (symbol/code/name), signDisplay choices, grouping on/off, min/max fraction digits, and examples that update live. |
| 3 | Accounting / spreadsheet format examples | Negative parentheses, optional plus signs, zero/negative sign policies, fixed decimal places, and western thousands separators. Some show Indian grouping as a separate locale option. |

## Table stakes → in-model / out-of-model

| Capability | Verdict | Where it landed |
|---|---|---|
| Format a raw number without exchange rates | in-model | The core never fetches rates; it formats the value only. |
| Currency marker as symbol or code | in-model | `currency` + `symbol_style=symbol|code|none`, with a known-code symbol table and literal fallback. |
| Marker before/after and optional space | in-model | `position=before|after` and `symbol_space` checkbox. |
| Decimal places and predictable money rounding | in-model | `decimals` 0–8 and `rounding` enum; digit-string rounding avoids float surprises such as `1.005`. |
| Thousands grouping and separator choices | in-model | `grouping`, `digit_grouping=western|indian`, and `group_separator` enum. |
| Decimal comma output | in-model | `decimal_separator=period|comma`, rejected if it conflicts with the active group separator. |
| Negative/accounting styles | in-model | `sign_style` enum plus `accounting` parentheses checkbox. |
| Trim trailing zeros | in-model | `trim_zeros` checkbox. |
| Locale database with currency names/plurals | out-of-model | This repo avoids bundling a full ICU/CLDR locale database for a simple pure block; controls expose the practical formatting knobs directly. |
| Currency conversion / live rates | out-of-model | Network data, rates, and historical conversions are explicitly not part of this presentation-only tool. |
| Arbitrary printf/Excel custom picture masks | out-of-model | Custom mask parsing is a separate formatter class; this tool focuses on safe explicit options. |

## Design decisions from the scan

1. Defaults match the common one-click USD case: `$1,234.50`.
2. Locale-like output is built from explicit controls, not a hidden locale database, so the page can represent EUR `1.234,50 EUR`, Indian `₹1,23,45,679`, and accounting `($1,234.50)` predictably.
3. Rounding modes are visible because copy/paste money workflows care about `half_up` vs `half_even`.
4. The input parser is forgiving about copied separators and currency affixes but strict about non-numeric text, so bad data is reported instead of silently reformatted.

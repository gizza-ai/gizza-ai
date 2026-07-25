# currency-converter — competitor analysis (2026-07-25)

Tool goal: convert amounts between currencies using a rate table supplied by the user, including multi-leg/cross-rate conversions. This tool intentionally does not fetch live exchange-rate data.

## Competitor scan (3 real tools skimmed)

1. **FreeCurrencyRates** — online converter with hundreds of currencies, code/name search, hourly-updated rates, and historical-rate views. Table-stakes: amount, from/to currencies, live rate freshness, historical rates, clear result formatting.
2. **FXTax multiple currency converter** — converts and compares multiple currencies at once, with reverse/cross-rate tables and historical trends. Table-stakes: reverse conversions, cross-rate calculations, multi-currency comparison, readable tables.
3. **Xe / OANDA currency converters** — common consumer UX: amount field, from/to currency pickers, swap/reverse behavior, live rates, rate timestamp/context, and a concise conversion result.

## Table-stakes → where each lands

| Table-stake | Decision |
|---|---|
| Amount field | in-model — `amount` param |
| Source and target currencies | in-model — `from` and `to` params |
| User-supplied rate table | in-model — `rates` multiline param |
| Direct conversion result with fixed decimal precision | in-model — `precision` param + rendered result |
| Reverse/inverted rates | in-model — `bidirectional` checkbox default true |
| Cross-rate / multi-leg conversion | in-model — shortest path through supplied rates, path reported |
| Flexible rate syntax | in-model — space, slash, colon, equals, or comma separators |
| Crypto or non-ISO tickers | in-model — 2–10 alphanumeric codes |
| Live exchange-rate feed / freshness timestamps | out-of-model — this public toolkit block is deterministic and offline; no network feed |
| Historical-rate lookup | out-of-model — needs a dated rate database or API |
| Multi-currency comparison table | out-of-model for this single-output tool; users can run one target at a time |
| Country/currency searchable picker | out-of-model — generic page controls are text fields; examples cover common codes |

## Design decisions

- **No live rates**: the tool is for receipt, invoice, historical, internal, or pasted crypto rates. Copy makes that explicit so users do not assume market data is fetched.
- **Bidirectional by default**: matches common converter swap behavior while allowing directional buy/sell rates by unchecking the box.
- **Multi-leg path reporting**: if USD→EUR and EUR→JPY are provided, USD→JPY works and reports `USD → EUR → JPY` so users can audit the cross rate.
- **Precision control**: defaults to 2 decimal places for money, with up to 10 for crypto/high-precision tables.

No competitor copy, branding, or trademarks were reproduced; out-of-model features are listed, not built.

# percentage-calculator — competitor analysis & surface checks (2026-06-29)

**Tool:** `percentage-calculator` — answer the five everyday percentage
questions (percent of a number, what percent, percent change, apply a change,
share of a total) from plain numbers. Pure compute, no network.

## Surface checks

Verification run on 2026-06-29 after implementation.

| Surface | Check | Result |
| --- | --- | --- |
| Core/workspace tests | `cd blocks/percentage-calculator && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 15 core tests + 1 descriptor drift guard passed |
| Chat block | `cd blocks/percentage-calculator && CARGO_BUILD_JOBS=1 wafer build` | ✅ `target/block.wasm` built and validated (319.1 KiB) |
| Web wasm | `wasm-pack build blocks/percentage-calculator/web --target web --release --out-dir pkg` | ✅ `web/pkg` built |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/percentage-calculator/` |
| CLI | `gizza tool percentage-calculator mode=percent_of percent=15 base=200` + `mode=change from=80 to=100` | ✅ summaries returned `15% of 200 = 30` and `25% increase` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-percentage-calculator.spec.ts` | ✅ 7 passed (5 modes + query-param deep-link + empty-state prompt) |

## Competitor scan

Searches to review:
- `percentage calculator online free top competitors CalculatorSoup RapidTables Omni Calculator.net`
- `percent change calculator percent of a number what percent of calculator competitors`

Representative competitors and references:

1. **Calculator.net — Percentage Calculator** — three-row layout ("what is X% of
   Y", "X is what percent of Y", "percentage change") plus a percentage-difference
   and percentage-change section.
2. **CalculatorSoup — Percentage Calculator** — multiple solved forms (P% of X,
   X is what % of Y, X is P% of what) with step-by-step explanations.
3. **RapidTables — Percentage Calculator** — compact percentage, percentage
   change, and percentage-difference calculators with formula notes.
4. **Omni Calculator — Percentage Calculator** — single "fill any two fields"
   percentage relationship plus a family of related percentage tools.
5. **Percentage-Calculator.net** — minimal three-question calculator (P% of X,
   X is what % of Y, percentage increase/decrease).

## Gap / fit analysis

| Capability | Competitors | gizza `percentage-calculator` | Decision |
| --- | --- | --- | --- |
| "What is P% of a number" | All five | ✅ `percent_of` (percent, base) | Built |
| "X is what percent of Y" | All five | ✅ `what_percent` (part, whole) | Built |
| Percent change (increase/decrease) | Calculator.net, RapidTables, Omni, Percentage-Calculator.net | ✅ `change` (from, to) returns percent + signed absolute change + direction | Built |
| Increase/decrease a number by P% | Calculator.net, Percentage-Calculator.net | ✅ `apply_change` (base, percent); negative percent decreases | Built |
| Share of a total / remaining | Spreadsheet-style and "tip/discount" variants | ✅ `percent_of_total` (value, total) returns share, remaining, remaining percent | Built |
| Negative inputs | Some calculators reject or mishandle | ✅ negatives allowed except as divisors; divisor-zero is a clear error | Built (edge correctness) |
| Structured/machine-readable output | Competitors render HTML only | ✅ JSON with echoed inputs, named measures + units, and a summary; same result across chat, CLI, page | Differentiator |
| Privacy / offline | Most run server- or ad-supported pages | ✅ pure wasm; nothing leaves the browser | Differentiator |
| Step-by-step worked solution | CalculatorSoup shows algebra steps | Partial: a human-readable `summary` line per result, no multi-step derivation | Good enough for current model |
| "X is P% of what" (solve for the base) | CalculatorSoup, Omni | ❌ not a separate mode; derivable as `base = part / (percent/100)` | Out of current scope |
| Tip / discount / VAT presets | Various niche calculators | ❌ out-of-model: domain presets layer on top of `apply_change` | Not built |

## Improvements made from analysis

- Covered the five questions that span the competitor set with one `mode` enum so
  the chat/CLI/page surfaces stay single-sourced from `descriptor()`.
- Returned signed absolute change and direction for `change`, and remaining /
  remaining-percent for `percent_of_total`, matching what the strongest
  competitors surface.
- Allowed negative inputs while still guarding divisor-zero with an explicit
  error message, avoiding the silent NaN/Infinity some web tools produce.
- Emitted structured JSON (echoed inputs + named measures with unit suffixes +
  summary) so results are machine-usable, not just visually rendered.
- Documented local/private execution in the page copy and FAQ.

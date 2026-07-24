# compound-interest-calculator — competitor scan & design decisions (2026-07-23)

Paraphrased scan only — no competitor copy, branding, or trademarks reproduced. Purpose:
fix the table-stakes parameter/UX set before implementing the gizza tool.

## Search

One WebSearch: "compound interest calculator online with monthly contributions". Skimmed the
top real competitor tools (paraphrased below); replaced an app-store result with reachable
web tools.

## Competitors reviewed (paraphrased)

1. **The Calculator Site — Compound Interest Calculator.** The most feature-complete of the
   set. Inputs: initial amount, interest rate (with a rate-period selector), a rich
   compounding-frequency list (daily 365/360, weekly, bi-weekly, semi-monthly, monthly,
   quarterly, half-yearly, yearly, continuous/custom), term as years **and** months, regular
   contribution amount with its own frequency, deposit-vs-withdrawal type, beginning/end-of-
   period timing, and an optional annual contribution-increase. Outputs: future value, total
   interest, compounded-vs-nominal rate, time-to-double, and monthly/yearly breakdown tables.

2. **Bankrate — Compound Savings Calculator.** Inputs: initial deposit, time period, APY,
   regular contribution with frequency (weekly / bi-weekly / monthly / annually), and
   compounding frequency (daily / monthly / quarterly / annually). Outputs: final balance,
   total contributions, interest earned.

3. **MoneyGeek — Compound Interest Calculator.** Monthly-or-annual contribution choice,
   compounding-frequency choice, and a year-by-year breakdown table plus a growth chart of
   contributions vs. interest vs. balance.

4. **MyFSB / Ultimate Finance Calculator (secondary reads).** Starting balance, monthly
   contribution, annual rate, term → future value, total deposited, total interest, and an
   effective-annual-rate readout.

## Table-stakes params (tagged)

| Param | In-model? | Decision |
| ----- | --------- | -------- |
| Initial principal / starting balance | in-model | `principal` (number, default 1000) |
| Annual interest rate (nominal %) | in-model | `annual_rate` (number, default 5) |
| Term in years (+ months) | in-model | `years` + `months` (numbers) |
| Compounding frequency | in-model | `compounding` enum: annually / semiannually / quarterly / monthly / weekly / daily / continuously |
| Regular contribution amount (deposit; negative = withdrawal) | in-model | `contribution` (number, default 0) |
| Contribution frequency | in-model | `contribution_frequency` enum: annually / semiannually / quarterly / monthly / biweekly / weekly |
| Contribution timing (start vs end of period) | in-model | `contribution_timing` enum: end / start |
| Future value output | in-model | `future_value` |
| Total contributions / total deposited | in-model | `total_contributions` |
| Total interest earned | in-model | `total_interest` |
| Effective annual rate (APY) | in-model | `effective_annual_rate` (%) |
| Year-by-year breakdown table | in-model | `schedule[]` (per-year balance / contributions-to-date / interest-to-date) |
| Growth chart (contributions vs interest visual) | out-of-model | The page renders JSON text; charting is the consuming site's concern. Listed, not built. |
| Currency symbol / locale selector ($ € £ ¥) | out-of-model (cosmetic) | Results are plain numbers; a currency glyph is presentation, injected site-side. Listed, not built. |
| Annual contribution increase (escalating deposits) | considered, rejected for v1 | Real table-stake at one competitor but adds schema surface; deferred to keep the first cut focused. Noted here so it is not silently dropped. |
| Time-to-double readout | in-model (nice-to-have) | Included as `years_to_double` in the result. |

## Model / math decision

Everything reduces to an **effective annual rate** first, which cleanly handles any
combination of compounding and contribution frequency:

- `EAR = (1 + r/n)^n − 1` for periodic compounding (`n` = compounding periods/yr), or
  `EAR = e^r − 1` for continuous compounding, where `r = annual_rate/100`.
- Principal grows by `(1 + EAR)^t` over `t = years + months/12`.
- Contributions use an annuity future-value at an equivalent contribution-period rate
  `i_c = (1 + EAR)^(1/m) − 1` (`m` = contribution periods/yr), with a `(1 + i_c)` multiplier
  when timing is start-of-period (annuity-due). Zero-rate falls back to `PMT · n`.
- `total_contributions = PMT · m · t`; `total_interest = future_value − principal −
  total_contributions`.

This is a standard, well-defined method, not copied from any competitor. Per-year schedule
rows are computed by evaluating the same closed form at each year boundary (capped).

## UX controls (page)

- `compounding`, `contribution_frequency`, `contribution_timing` → `Param::enumv` → native
  `<select>` with friendly `[input.labels]`.
- `annual_rate` → `kind = "slider"` (0–20%, step 0.1) mirrored onto the number box.
- Numeric fields pre-filled with real defaults + placeholders so the page shows a worked
  result on load.
- `[[example]]` preset chips (starter savings, retirement drip, lump-sum only) — the
  declarative answer to competitors' preset buttons; they double as worked examples.

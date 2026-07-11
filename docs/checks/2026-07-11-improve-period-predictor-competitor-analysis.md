# period-predictor — competitor analysis (2026-07-11)

Paraphrased scan of the top real period / cycle calculators. No competitor copy,
branding, or trademarks reproduced — findings are summarized in our own words.

## Competitors skimmed

1. **calculator.net — Period Calculator** — inputs: first day of last period,
   average cycle length (default 28), period (bleeding) duration (default 5).
   Outputs a multi-month calendar of predicted period days, estimated ovulation
   day, and the fertile window, projected across several future cycles.
2. **Omni Calculator — Period Calculator** — inputs: last period date, cycle
   length, period length. Outputs next period date, current cycle day, fertile
   window, ovulation day. States results are estimates and not a contraceptive
   method.
3. **Flo / Natural Cycles — Period Calculator** — inputs: last period start,
   average cycle length. Predicts the next several period start dates, ovulation
   (~14 days before the next period), and the 6-day fertile window (5 days before
   ovulation + ovulation day). Notes the luteal phase is roughly constant
   (12–16 days) regardless of total cycle length.

## Table-stakes params / features (tagged in-model / out-of-model)

| Feature | In-model? | Decision |
|---|---|---|
| Last period start date | in-model | `last_period` (required date) |
| Average cycle length (default 28) | in-model | `cycle_length` slider, default 28, 20–45 |
| Period / bleeding duration (default 5) | in-model | `period_length` slider, default 5, 1–14 → period end date |
| Number of future cycles to predict (several months) | in-model | `cycles` slider, default 6, 1–24 |
| Ovulation day (~14 d before next period) | in-model | derived; `luteal_phase` slider, default 14, 9–17 |
| Fertile window (5 days before ovulation + ovulation) | in-model | derived per cycle |
| Weekday of each predicted start | in-model | derived (nicety competitors show on their calendar) |
| Visual month-grid calendar | out-of-model | Listed, not built — page is a text/JSON tool, not an interactive calendar renderer. Dates + weekdays cover the same information. |
| Symptom / mood / flow logging + history | out-of-model | Out of scope — this is a stateless predictor, not a tracker/app with storage. |
| Pregnancy / due-date estimation | out-of-model | Separate tool concern; not built here. |

## Defaults chosen (match mainstream calculators)

- `cycle_length` = 28 days, `period_length` = 5 days, `luteal_phase` = 14 days,
  `cycles` = 6.
- Ovulation date for a cycle = its period start − `luteal_phase`.
- Fertile window = ovulation − 5 days … ovulation day (6-day window).

## UX control patterns adopted

- Native **date picker** for `last_period` (`kind = "date"`).
- **Sliders** for `cycle_length`, `period_length`, `luteal_phase`, `cycles`
  (bounded numeric ranges — dragging beats typing; matches competitor +/- steppers).
- **Preset example chips** (`[[example]]`) for a standard 28-day cycle and a
  short 24-day cycle, mirroring competitors' quick presets.

## Worked example

Last period `2026-07-01`, cycle 28, period 5, luteal 14, 3 cycles →
next period `2026-07-29` (Wednesday), then `2026-08-26`, `2026-09-23`; each with
its bleeding-end date, ovulation (~14 d prior), and 6-day fertile window.

## Limitations stated on the page

- Predictions are **estimates** — real cycles vary a few days with stress,
  illness, travel, etc.
- **Not a contraceptive method** and not medical advice; irregular cycles are
  less predictable.

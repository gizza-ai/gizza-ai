# tdee-calculator — competitor analysis (2026-07-27)

Snapshot taken before implementing. Sources are real public TDEE/BMR calculators; all copy here
is paraphrased — no competitor text, branding, or trademarks are reproduced.

## Competitors scanned (top 3)

1. **calculator.net — TDEE Calculator** (`calculator.net/tdee-calculator.html`)
2. **tdeecalculator.net** (`tdeecalculator.net`)
3. **Inch Calculator — Mifflin-St Jeor Calculator** (`inchcalculator.com/mifflin-st-jeor-calculator/`)

(NutriAdmin, tdeecalculator.org and CalcoI corroborate the same formula + multiplier set.)

## Table-stakes inputs (and where each landed)

| Capability | Competitors | Decision |
|---|---|---|
| Age (years) | all | **in-model** — `age` param |
| Sex / gender (male/female) | all | **in-model** — `sex` enum |
| Weight | all | **in-model** — `weight` + `units` |
| Height | all | **in-model** — `height` + `units` |
| Metric / imperial units | all | **in-model** — `units` enum (metric = kg/cm, imperial = lb/in) |
| Activity level (5-tier + multipliers) | all | **in-model** — `activity` enum → 1.2 / 1.375 / 1.55 / 1.725 / 1.9 |
| BMR formula: Mifflin-St Jeor (default) | all | **in-model** — `formula` enum |
| BMR formula: Harris-Benedict (revised) | calculator.net, inch | **in-model** — `formula` enum |
| BMR formula: Katch-McArdle (uses body fat) | calculator.net, tdeecalculator.net | **in-model** — `formula` enum + `body_fat` |
| Body-fat % (Katch-McArdle) | calculator.net, tdeecalculator.net | **in-model** — `body_fat` param |
| Result unit: calories or kilojoules | calculator.net | **in-model** — `energy_unit` enum |
| TDEE shown across all activity levels | inch, tdeecalculator.net | **in-model** — `tdee_by_activity` output |
| Goal calories (cut / maintain / bulk) | calculator.net, tdeecalculator.net | **in-model** — `goals` output (±250/±500/−1000) |
| BMI (bonus, derivable from weight+height) | tdeecalculator.net | **in-model** — `bmi` + `bmi_category` output |
| Macro breakdown (protein/carb/fat grams) | tdeecalculator.net | **out-of-model for now** — depends on an opinionated macro split with no single standard; documented as a limit, not silently dropped |
| Feet+inches as two separate fields | calculator.net, tdeecalculator.net | **folded** — imperial height is entered in total inches (single numeric field); documented on the page |

## Formulas used (standard, public-domain equations)

- **Mifflin-St Jeor (1990):** BMR = 10·kg + 6.25·cm − 5·age + (5 male / −161 female)
- **Harris-Benedict (Roza–Shizgal 1984 revision):**
  - men: 88.362 + 13.397·kg + 4.799·cm − 5.677·age
  - women: 447.593 + 9.247·kg + 3.098·cm − 4.330·age
- **Katch-McArdle:** BMR = 370 + 21.6·LBM, LBM = kg·(1 − body_fat/100) (age/sex/height unused)
- **Activity multipliers:** sedentary 1.2, light 1.375, moderate 1.55, very active 1.725, extra active 1.9
- **Goal offsets:** mild loss −250, loss −500, extreme loss −1000, mild gain +250, gain +500 kcal/day
  (≈0.25/0.5/1 kg per week; floored at 0 and flagged as estimates, not medical advice)
- **kJ conversion:** 1 kcal = 4.184 kJ

## UX / control patterns matched

- `activity`, `formula`, `sex`, `units`, `energy_unit` render as friendly `<select>`s (`[input.labels]`).
- `age`, `weight`, `height`, `body_fat` render as numeric fields with placeholders.
- `[[example]]` preset chips (competitors ship quick presets): a metric maintenance case, an
  imperial cutting case, and a Katch-McArdle athlete case.

## Out-of-model / deliberately excluded

- **Macro (protein/carb/fat) split** — no single canonical ratio; would be an opinionated add-on.
  Listed as a page limit.
- **Lean-body-mass estimation from measurements** (Navy tape method) — separate tool territory.
- No account, no data upload — the tool is pure and runs locally.

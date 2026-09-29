# calorie-burn — competitor analysis (2026-09-24)

Scan run BEFORE implementing, per `/create-next-tool` step 4. One WebSearch
("calories burned calculator activity MET duration body weight") plus a fetch of the top three
reachable competitor tools. All notes are **paraphrased** observations of feature surface —
no competitor copy, wording, branding or trademarks are reproduced or reused.

## Competitors reviewed

| # | Tool | URL |
|---|------|-----|
| 1 | Calculator.net — calories burned calculator | https://www.calculator.net/calories-burned-calculator.html |
| 2 | Omni Calculator — calories burned calculator | https://www.omnicalculator.com/sports/calories-burned |
| 3 | METS Calculator — calories burned / METs | https://metscalculator.com/ |

## What each one offers

**1. Calculator.net.** Two calculators on one page: a duration-based one and a
distance-based one. Duration mode takes an activity from a grouped dropdown (walking, running,
cycling, swimming, gym, sports, outdoor, household — 80+ entries), a duration as separate hours
and minutes fields, and a body weight with a lb/kg toggle (lb 80–350, kg 35–160). Distance mode
takes activity (walking/running/cycling only), a speed or pace with six unit choices, and a
distance with four unit choices. States the formula as `minutes × MET × kg / 200`. Output is a
single calorie figure. Explicitly frames the result as an estimate against an "average" reference
person and notes that MET values assume a constant work rate.

**2. Omni Calculator.** Takes weight (unit-toggleable), an activity from a ~40-entry dropdown,
a duration, and — the interesting part — an **editable MET field** that pre-fills from the chosen
activity so the user can dial intensity up or down. Has a "compare two activities" mode. Formula
shown as `T × MET × 3.5 × W / (200 × 60)` with T in seconds. Outputs total energy, **energy per
hour**, and a **body-mass-loss equivalent** derived by dividing by 7700 kcal/kg. Its own worked
example: 90 kg, 7 h, MET 9.5 → ~6284 kcal, ~897.7 kcal/h, ~0.82 kg. Says it ignores intensity
beyond the MET value and that MET values are population averages. FAQ block answers several
"how many calories does X burn" questions with concrete numbers.

**3. METS Calculator.** Weight with kg/lb toggle, activity time with min/hr toggle, and a
two-level activity selector (category → description). Uses the simpler `kg × MET × hours` form
and states the 1 MET = 1 kcal/kg/h ≈ 3.5 ml O₂/kg/min identity. Outputs total calories, the
activity echoed back, and the **MET value itself**. Explicitly notes the formula ignores age and
sex. No FAQ, no weight-loss projection, no MET-minutes.

## Table-stakes matrix

| Capability | Seen on | Fit | Decision |
|---|---|---|---|
| Activity picker with compendium MET values | 1, 2, 3 | in-model | **Built** — `activity`, 44 curated entries + `custom`, each labelled with its MET |
| Editable / override MET value | 2 | in-model | **Built** — `met` (0 = use the activity's value; any value 0.5–30 overrides it) |
| Body weight with kg/lb toggle | 1, 2, 3 | in-model | **Built** — `weight` + `weight_unit` (`kg`/`lb`), both echoed in the output |
| Duration with minutes/hours handling | 1, 2, 3 | in-model | **Built** — `duration` + `duration_unit` (`minutes`/`hours`) |
| Formula stated on the page | 1, 2, 3 | in-model | **Built** — worked example shows `MET × 3.5 × kg / 200 × minutes` end to end |
| Calories per hour / per minute | 2 | in-model | **Built** — both reported alongside the session total |
| MET value echoed in the result | 3 | in-model | **Built** — value plus its source (compendium entry vs. user override) |
| Body-fat / weight-loss equivalent | 2 | in-model | **Built** — grams of body fat at 7700 kcal/kg |
| Compare activities side by side | 2 | in-model | **Built** — every output includes a comparison block: the same weight and duration across 8 reference activities (broader than the competitor's two-activity compare) |
| Estimate-only caveat, stated limits | 1, 2, 3 | in-model | **Built** — "Limits and edge cases" section on the page, plus a FAQ entry |
| Unit toggles as real controls | 1, 2, 3 | in-model | **Built** — `enumv` params render as selects with friendly `[input.labels]` |
| Preset buttons / common scenarios | — (competitors use dropdown defaults instead) | in-model | **Built anyway** — six `[[example]]` chips (30-min walk, 5 k run, gym session, swim, desk day, custom MET) |
| **Net vs. gross calories** | none of the three | in-model | **Built as a differentiator** — `basis` (`gross`/`net`); net subtracts the ~1 MET resting cost, which is the honest number for "extra" calories |
| **MET-minutes of activity volume** | none of the three | in-model | **Built as a differentiator** — MET-minutes plus how many such sessions reach the 500 MET-min/week public-health floor |
| **Oxygen uptake (VO₂)** | 3 mentions the identity only | in-model | **Built as a differentiator** — ml/kg/min and total litres of O₂ for the session |
| Distance/pace-based mode (enter km + pace instead of duration) | 1 | in-model but **out of scope** | **Listed, not built** — it is a second calculator with its own six pace units and three activities; it belongs in a separate distance/pace tool rather than doubling this one's parameter surface |
| Grouped/two-level activity dropdown (category → description) | 1, 3 | **out-of-model** | **Listed, not built** — the page form renders one `<select>` per param; the generator has no `<optgroup>` control kind. Mitigated by category-prefixed labels ("Running — 6 mph…") so the flat list still reads as grouped |
| Age/sex/height-aware personalisation | none (all three disclaim it) | out-of-model for *this* tool | **Listed, not built** — MET arithmetic is weight-and-duration only by definition; `tdee-calculator` already covers age/sex/height BMR-based expenditure |
| Heart-rate-based calorie estimation | — | out-of-model for this tool | **Listed, not built** — needs HR data; `heart-rate-zones` is the adjacent tool |
| Food-equivalent outputs ("= 1 doughnut") | 2's FAQ gestures at it | in-model but declined | **Listed, not built** — brand-dependent food data, no stable source, and it would date badly |
| Shareable result link | 2 | already covered | The generated page supports `?param=` deep links for every field, so a filled-in run is already a URL |

## Duplicate check

`ls blocks/ | grep -iE 'calor|met|burn|activ|exercise|bmr|tdee'` surfaced `tdee-calculator`,
`heart-rate-zones` and `ideal-weight`. Not duplicates:

- `tdee-calculator` computes **basal** metabolic rate (Mifflin/Harris-Benedict on age, sex,
  height, weight) and scales it by a lifestyle activity factor to get a **whole-day** figure. It
  has no MET table and no per-session duration input.
- `calorie-burn` computes the cost of **one specific bout of activity** from its MET value, the
  body weight and the elapsed time. Different formula, different inputs, different question.
- `heart-rate-zones` and `ideal-weight` are unrelated.

## Notes on MET values

Values are the standard Compendium-of-Physical-Activities figures that every competitor above
draws on (e.g. slow walking 2.8, 6 mph running 9.8, moderate cycling 8.0, vigorous lap swimming
9.8). They are a published reference table, not competitor content. Where sources give a range
for an intensity band, the mid-band value is used and the label names the intensity so the user
can override it with `met`.

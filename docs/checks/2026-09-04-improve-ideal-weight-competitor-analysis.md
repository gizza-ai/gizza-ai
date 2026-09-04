# ideal-weight — competitor analysis (2026-09-04)

Scan run **before** implementing, per the create-next-tool loop. All findings are paraphrased
observations of publicly visible tool behaviour — **no competitor copy, branding, or trademarks
were reproduced**. Out-of-model items are recorded, not built.

## Competitors reviewed

| # | Tool | What it does |
|---|------|--------------|
| 1 | calculator.net — ideal weight calculator | Age + gender + height (US/metric toggle); shows a comparison table of four classic formulas plus a healthy-BMI weight range; links CDC percentile charts for ages 2–20 |
| 2 | miniwebtool — ideal weight calculator | Gender + height (ft/in ↔ cm toggle) + body-frame size + optional age; outputs the four formulas, their average, the min–max spread, a healthy-BMI range, a bar chart, and a wrist-circumference frame reference table; ±10% frame adjustment; preset example links; embeddable widget |
| 3 | fatcalc — IBW calculator | Height (imperial/metric) + sex + frame size dropdown with wrist guidance; four formulas side by side, BMI-based healthy range, frame-adjusted values; explicitly documents that the formulas are anchored at a 5-foot baseline and go meaningless (even negative) far below it |

A fourth/fifth tier (ajdesigner, icalculator, wellistic, mymathtables) repeats the same feature
set — four formulas, sex, height, sometimes frame — so three profiles cover the table stakes.

## Formula definitions the field agrees on (kg, height in inches)

| Formula | Male | Female |
|---|---|---|
| Hamwi (1964) | 48.0 + 2.7 × (in − 60) | 45.5 + 2.2 × (in − 60) |
| Devine (1974) | 50.0 + 2.3 × (in − 60) | 45.5 + 2.3 × (in − 60) |
| Robinson (1983) | 52.0 + 1.9 × (in − 60) | 49.0 + 1.7 × (in − 60) |
| Miller (1983) | 56.2 + 1.41 × (in − 60) | 53.1 + 1.36 × (in − 60) |

All three competitors implement exactly these coefficients, so they are the correctness baseline
(the core unit tests assert each one at 70 in / 64 in).

## Table stakes → decision

| Capability | Seen at | Verdict |
|---|---|---|
| All four formulas side by side, not one | 1, 2, 3 | **Built** — `formulas[]` returns Hamwi/Devine/Robinson/Miller together |
| Sex selection (male/female) | 1, 2, 3 | **Built** — `sex` enum |
| Metric ↔ imperial height entry | 1, 2, 3 | **Built** — `units` enum (cm, or total inches, matching the `tdee-calculator` family convention) |
| Healthy weight range from BMI | 1, 2, 3 | **Built** — `healthy_bmi_range` with weights at both BMI bounds |
| Body-frame ±10% adjustment | 2, 3 | **Built** — `frame` enum small/medium/large plus `auto` |
| Wrist-circumference → frame reference table | 2, 3 (static table) | **Built, better** — `frame = auto` derives the frame from `wrist` + height using the standard clinical table, instead of making the user read a table |
| Average of the formulas + min–max spread | 2 | **Built** — `average_kg/lb` and `range_kg/lb` |
| Results in kg **and** lb regardless of input units | 2, 3 | **Built** — every weight is reported in both |
| Optional age field | 1, 2 | **Built** — `age` drives a caveat note for under-18 (the formulas are adult-only); it is not an input to any formula, and the tool says so |
| Preset example links | 2 | **Built** — four `[[example]]` chips |
| Short-stature / negative-value caveat | 3 (prose only) | **Built, better** — heights below 122 cm / 48 in are rejected with an explanatory error; 122–152 cm results carry an extrapolation note |
| Adjustable healthy-BMI bounds | none | **Built (beyond baseline)** — `bmi_min`/`bmi_max` default to 18.5/24.9 but can be set to e.g. 18.5–23 for the WHO Asian cutoffs |
| Formula results expressed as BMI | none | **Built (beyond baseline)** — each formula row carries `bmi_at_ideal`, which makes the disagreement between formulas legible |

## Considered, not built (out-of-model)

- **Bar-chart comparison of the formulas** (2). The page generator renders text/JSON output; there
  is no declarative chart control, and adding one for a single tool would be a per-tool hack. The
  same information is in `formulas[]` and `range_kg`.
- **CDC pediatric BMI percentile charts for ages 2–20** (1). Genuinely a different tool and a
  different data model (LMS reference tables); this repo already has `child-growth-percentile`.
  The `age` note points under-18 users at growth charts rather than faking it here.
- **Embeddable widget / saved calculations** (2). Needs a host, an account, or an iframe embed
  story — outside the browser-local, no-account model.
- **Body-composition-aware targets** (body fat, lean mass). `tdee-calculator` already covers the
  Katch-McArdle lean-mass path; duplicating it here would be schema bloat.

## Honest limits recorded on the page

The formulas were built for clinical drug dosing and dietetics, not as personal health targets;
they disagree by several kg at the same height, ignore body composition, ethnicity and age, and
are calibrated for adults. All of that is stated in `page/content.md`, not just in code.

# child-growth-percentile competitor analysis (2026-08-23)

Backlog row: `child-growth-percentile` — compute a child's height/weight/BMI percentile from WHO/CDC growth charts.

Research query used by the builder: child growth percentile calculator CDC height weight BMI head circumference.

## Competitor scan

| Competitor class | Observed table stakes | Fit decision |
|---|---|---|
| Public-health growth calculators | Ask for sex, age, height/length, weight and sometimes head circumference; report percentiles against CDC or WHO references. | In model for CDC references using bundled public-domain LMS coefficients. WHO reference selection is out of scope for this first tool and listed as a limit. |
| Paediatric BMI percentile tools | Accept age and sex, compute BMI from height/weight, report BMI-for-age percentile and broad CDC weight category. | In model for ages 2–20 with CDC BMI-for-age LMS data and screening category text. |
| Infant growth tools | Handle birth-to-36-month length, weight and head circumference charts separately from child stature charts. | In model. `chart=auto` switches at 24 months; `chart=infant` can be forced through 36 months. |
| Clinical EHR calculators | Store patients, plot longitudinal growth curves, flag clinical workflows and print branded reports. | Out of model. This repo is a stateless local calculator; no patient storage, no diagnosis, no longitudinal chart. |
| International growth tools | Offer WHO, UK/RCPCH and country-specific references. | Out of model for now. The bundled tables are CDC-only to avoid mixing references silently. |

## Controls and defaults

| Capability | Control/default | In model? | Decision |
|---|---:|---|---|
| Sex-specific references | `sex=boy|girl` | Yes | Required enum. |
| Flexible age input | `age`, bare number = months | Yes | Accepts months, years/months, days/weeks and date ranges. |
| Metric and US units | `units=metric|us` | Yes | Converts US inches/pounds to cm/kg internally. |
| Infant vs child chart selection | `chart=auto|infant|child` | Yes | Auto uses infant before 24 months and child from 24 months. |
| Height/length percentile | `height` | Yes | Uses length-for-age or stature-for-age depending on chart. |
| Weight percentile | `weight` | Yes | Uses weight-for-age where the selected chart covers the age. |
| BMI percentile/category | `height` + `weight` | Yes | Available from 24 months on child charts. |
| Head circumference | `head_circumference` | Yes | Available on infant charts through 36 months. |
| Decimal precision | `decimals=2`, range 0–4 | Yes | Prevents noisy over-precision. |
| WHO/UK references and diagnosis | n/a | No | Documented as limits; not built. |

## Worked examples selected

- 3-year-old girl with height and weight to exercise height, weight, BMI and weight-for-stature.
- Newborn boy with head circumference to exercise infant charts.
- US-units example to exercise inches/pounds conversion.
- Date-range age example to exercise alternate age parsing.

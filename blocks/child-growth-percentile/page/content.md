## About this tool

Use this calculator when you need a reproducible CDC growth-chart lookup without uploading a child's measurements. It applies the CDC LMS coefficients bundled with the tool to estimate percentiles and z-scores for height/length-for-age, weight-for-age, BMI-for-age, head-circumference-for-age, and weight-for-length or weight-for-stature when the requested chart covers the child's age and measurement range.

Age can be entered as months (`36`), years and months (`3y 4m`), days or weeks (`95 days`, `6 weeks`), or a date range such as `2023-04-15 to 2026-08-23`. Measurements can be metric (centimetres and kilograms) or US customary (inches and pounds). Put `0` in any measurement field you did not take.

### Worked example

Input:

- Sex: `girl`
- Age: `3y`
- Height: `95`
- Weight: `14`
- Units: `metric`
- Chart: `auto`

Output includes:

```text
Child growth percentiles - CDC growth charts
Child: girl, age 3 y 0 mo (36 months)
Reference: CDC 2-20 years growth charts (standing stature)

Height-for-age: 95 cm -> ...th percentile (z = ...)
Weight-for-age: 14 kg -> ...th percentile (z = ...)
BMI-for-age: 15.51 kg/m2 -> ...th percentile (z = ...)
Weight-for-stature: 14 kg at 95 cm -> ...th percentile (z = ...)
```

The exact percentile depends on the CDC LMS row interpolation and the selected decimal precision.

### What is calculated

| Measurement supplied | CDC reference reported | Notes |
| --- | --- | --- |
| Height/length | Length-for-age for infant charts, height/stature-for-age for child charts | Infant charts use recumbent length; 2–20 year charts use standing stature. |
| Weight | Weight-for-age | Available for the chart's age span. |
| Height + weight | BMI-for-age, plus weight-for-length/stature | BMI-for-age starts at 24 months. |
| Head circumference | Head-circumference-for-age | CDC head circumference chart covers birth through 36 months. |

### Limits and edge cases

- CDC age range is birth through **240 months** (20 years). Infant charts cover birth through **36 months**; child charts start at **24 months**.
- At least one of height, weight or head circumference must be greater than zero.
- Percentiles are rounded to 0–4 decimal places.
- Measurements outside a chart's reference range are reported as unavailable instead of extrapolated.
- This is a screening/reference calculator, not a diagnosis. Growth should be interpreted over time and with clinical context.

## FAQ

<details>
<summary>Which growth charts does this use?</summary>

It uses CDC LMS growth-chart coefficients: infant charts for birth to 36 months and 2–20 year charts for older children. The coefficients are bundled in the WebAssembly tool, so calculation happens locally without a network call.

</details>

<details>
<summary>Should I choose infant, child, or auto?</summary>

Use **auto** for most cases. It uses infant references below 24 months and 2–20 year references from 24 months onward. Force **infant** only when you specifically need the birth–36 month charts, and force **child** only when you specifically need the 2–20 year charts.

</details>

<details>
<summary>Why are height and length treated differently?</summary>

Infant charts use recumbent length, while the 2–20 year charts use standing stature. The number goes in the same field, but the selected chart determines whether the report labels it as length-for-age or height-for-age.

</details>

<details>
<summary>Can this diagnose underweight, overweight, or a medical problem?</summary>

No. The report gives reference percentiles and z-scores. Clinical interpretation depends on measurement technique, repeated measurements, growth velocity, puberty, medical history and local clinical guidance. Discuss concerns with a paediatric clinician.

</details>

<details>
<summary>Why does one of my measurements say it is unavailable?</summary>

Some CDC references cover only certain ages or measurement ranges. For example, head circumference stops at 36 months and BMI-for-age starts at 24 months. The tool reports that limit instead of extrapolating beyond the published chart.

</details>

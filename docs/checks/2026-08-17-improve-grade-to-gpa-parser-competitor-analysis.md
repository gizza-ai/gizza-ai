# grade-to-gpa-parser — competitor scan + design decisions (2026-08-17)

Backlog row: `grade-to-gpa-parser` / "Maps letter grades (A, B+, C-) to GPA points on a configurable scale and averages them." / `pure`.

## Duplicate check

Existing adjacent tools include general calculators, statistical tools, and education-adjacent parsers, but no block maps transcript letter grades to GPA points with credits and weighted-course handling. No semantic duplicate was found; build proceeds.

## Competitors reviewed

### Calculator.net GPA Calculator
- Accepts course rows with letter grade and credits.
- Includes plus/minus letter choices, credit weighting, and a cumulative GPA section.
- UX pattern: table-like rows and examples for common transcript entries.

### CollegeSimply GPA Calculator
- Provides a course list where each row has a grade and credits.
- Supports common A through F grade values and returns total GPA.
- UX pattern: clear defaults, fast result, and explanation of GPA meaning.

### Scholaro GPA Calculator
- Focuses on configurable grading scales and international grade conversions.
- Lets users map grades to grade points, then computes a weighted average.
- UX pattern: expose the scale as editable/mappable data rather than hard-coding one school.

## Table stakes → decision

| Capability | Verdict | How it lands here |
| --- | --- | --- |
| Parse letter grades including plus/minus | in-model — built | `grades` accepts A+, A, A-, B+ … F/E |
| Weighted average by credits | in-model — built | Per-entry credits plus `default_credits` |
| 4.0 scale | in-model — built | `scale=4.0` default |
| A+ as 4.3 option | in-model — built | `scale=4.3` |
| Weighted 5.0 style scale | in-model — built | `scale=5.0`, plus AP/honors bonuses |
| Custom school scale | in-model — built | `custom_scale` accepts `LETTER=POINTS` overrides |
| Percentage input | in-model — built | `grade_format=percent` or `auto` |
| Raw point input | in-model — built | `grade_format=points` or `auto` |
| Prior GPA / cumulative GPA | in-model — built | `prior_gpa` and `prior_credits` |
| Pass/fail / withdrawal rows | in-model — built | `skip_nongraded` lists and excludes P/W/etc. |
| Detailed course breakdown | in-model — built | Report lists each course, grade, credits, and quality points |
| JSON export | in-model — built | `output=json` |
| Institution-specific official policy database | out-of-model | Policies vary by school and date; users can encode their scale with `custom_scale` |
| Transcript PDF import | out-of-model | Different input class; this tool is paste-in text only |

## UX patterns adopted

- One multiline input so users can paste a transcript-like list.
- Select controls for scale, grade format and output.
- Numeric controls for credits, bonuses, prior GPA and decimals.
- A checkbox for pass/fail handling, with a non-default-state test.
- Preset examples for a simple list, credit-weighted transcript rows, AP/honors weighting, cumulative GPA, and JSON output.

## Not copied

No competitor text, branding, examples, layout, or assets were reused. Scale names and grade letters are domain vocabulary and are used descriptively.

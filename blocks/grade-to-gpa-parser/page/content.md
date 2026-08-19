## About this tool

Grade to GPA Parser turns a pasted grade list into GPA points and a credit-weighted average. Paste bare grades such as `A, B+, C-`, or transcript-like rows such as `Biology: A- 4` and `AP History: B+ (3)`. The report shows the GPA, total grade points, credits counted, every course's contribution, skipped pass/fail rows, and optional cumulative GPA.

Use the default 4.0 scale for common US unweighted GPA calculations. Switch to 4.3 when your school gives A+ extra weight, choose 5.0 for a weighted scale, or add `custom_scale` overrides such as `A+=4.5, HD=4.0` for local grading systems.

### Worked example

Input:

```text
Biology: A- 4
Math: B 3
Art: C 1
```

With the default 4.0 scale, the tool computes `(3.7×4 + 3.0×3 + 2.0×1) / 8 = 3.225`, reported as `GPA: 3.23` when `decimals=2`.

```text
GPA: 3.23
Grade points: 25.80
Credits counted: 8.00
Courses counted: 3
```

### Limits and edge cases

- Accepts up to 2,000 grade entries per run.
- Entries may be separated by newlines, semicolons, or commas.
- A number above the top of the selected scale is treated as a percentage in `auto` mode; a number at or below it is treated as grade points.
- Pass/fail, withdrawal, incomplete, audited, transfer, and similar marks are excluded by default and listed under "Not counted".
- Weighted-course bonuses never lift a failing grade.
- This is a calculator, not official academic advice; always compare the scale against your school policy.

## FAQ

<details>
<summary>How do I enter credits?</summary>

Put the credit value after the grade, for example `Biology: A- 4` or `AP History: B+ (3)`. If an entry has no credit value, the `default_credits` field is used.

</details>

<details>
<summary>Can I use my school's custom grading scale?</summary>

Yes. Pick the closest base scale, then add overrides in `custom_scale`, such as `A+=4.5, A=4.2, B+=3.6`. Unknown letters are added and existing letters are replaced.

</details>

<details>
<summary>What happens to pass/fail or withdrawal marks?</summary>

With `skip_nongraded=true`, marks such as P, W, I, CR, NC, AU, and TR are listed but excluded from the GPA. Turn the checkbox off if you prefer the tool to error when it sees one.

</details>

<details>
<summary>How are percentages handled?</summary>

In `auto` mode, values above the selected scale's maximum are treated as percentages and mapped through common plus/minus bands. Use `grade_format=percent` to force every number through the percentage bands, or `grade_format=points` to treat numbers as grade points.

</details>

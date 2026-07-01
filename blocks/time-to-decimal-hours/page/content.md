# Time to Decimal Hours Converter

Convert work durations between clock notation and decimal hours.

## What it does

- `1:30` becomes `1.5` decimal hours.
- `1.5` becomes `1:30`.
- `2:15:18` is supported when seconds matter.
- Negative durations such as `-0:30` and `-0.5` are handled.
- The output also includes total minutes and total seconds for payroll, timesheets, billing, and spreadsheet checks.

## Inputs

- **Duration** — enter either `H:MM`, `H:MM:SS`, or a decimal number of hours.
- **Interpret input as** — leave `auto` to detect from the presence of `:`, or force `from-clock` / `from-decimal` when validating a specific format.

Everything runs locally in your browser; no time data leaves your machine.

## FAQ

<details>
<summary>Why does 1:20 convert to 1.3333 and not 1.20?</summary>

Because decimal hours are *minutes ÷ 60*, not the minutes written after a
point: 20 minutes is 20/60 = 0.3333 of an hour. This is the classic timesheet
mistake — `1.20` decimal hours actually means 1 hour 12 minutes. If your
payroll sheet expects decimal hours, always convert rather than swapping `:`
for `.`.

</details>

<details>
<summary>How does it decide whether my input is clock time or decimal?</summary>

In `auto` mode (the default) a value containing a colon is parsed as `H:MM` or
`H:MM:SS`, anything else as decimal hours. If you want strict validation —
say, catching `1.30` typed where `1:30` was meant — force the interpretation
with **from-clock** or **from-decimal** and malformed input becomes an error
instead of a silent reinterpretation.

</details>

<details>
<summary>Can I enter durations over 24 hours, or negative ones?</summary>

Yes to both. These are *durations*, not times of day, so `100:30` (a hundred
hours and change) is fine, and a leading `-` (e.g. `-0:30` or `-0.5`) carries
through to every output. The only bounds are on components: minutes and
seconds must be 0–59.

</details>

<details>
<summary>How is rounding handled?</summary>

Everything is canonicalised through a whole number of seconds — decimal input
is rounded to the nearest second — and the reported decimal hours and total
minutes are rounded to 4 decimal places. That keeps results deterministic and
means the clock form, decimal form, and totals always describe exactly the
same duration.

</details>

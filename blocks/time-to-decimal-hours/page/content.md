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

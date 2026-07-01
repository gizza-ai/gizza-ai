## About this tool

**Extract Dates** scans a block of text, finds every date and time it mentions,
and lists them **normalized to ISO 8601** in the order they appear. Each result
shows the original text, the normalized value, and whether it's a date, datetime,
or time.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Recognized formats

- **ISO** — `2024-01-05`, `2024-01-05T14:30`, `2024-01-05 14:30:00`
- **Numeric** — `01/05/2024`, `3-4-99` (read **month-first / US style** unless
  the first field is greater than 12, e.g. `25/12/2024` → 25 December)
- **Year-first** — `2024/01/05`
- **Month name** — `January 5, 2024`, `5 Jan 2024`, `Dec. 25 1999`
- **Clock times** — `14:30`, `2:30:15`, `3pm`, `9:05 AM` (normalized to
  24-hour `HH:MM:SS`)

Invalid dates and times (e.g. `Feb 30`, `25:99`) are skipped automatically.

### A note on ambiguity

Purely numeric dates like `01/05/2024` are inherently ambiguous between
month-first (US) and day-first (most of the world). This tool assumes
**month-first** unless the first number can only be a day (> 12). For
unambiguous results, prefer ISO `YYYY-MM-DD`.

## FAQ

<details>
<summary>Can I make numeric dates parse day-first (European style)?</summary>

Not with a switch — the parser is fixed to **month-first** and only reads
day-first when the first number is greater than 12 (so `25/12/2024` is
correctly 25 December). If your text uses day-first dates with days ≤ 12,
the safest fix is converting them to ISO `YYYY-MM-DD` before extracting.

</details>

<details>
<summary>How are two-digit years like 3/4/99 interpreted?</summary>

With a fixed pivot: `00`–`69` become 20xx and `70`–`99` become 19xx, so
`3/4/99` normalizes to `1999-03-04` and `3/4/25` to `2025-03-04`. Four-digit
years are always taken literally.

</details>

<details>
<summary>Why is a date next to a time reported as one datetime?</summary>

When a date and time are adjacent in an ISO-style form (`2024-01-05 14:30` or
with a `T`), they're matched together and reported as a single `datetime` in
`YYYY-MM-DDTHH:MM:SS` form. Standalone clock times — including 12-hour ones
like `3pm` or `9:05 AM` — are reported separately as `time`, normalized to
24-hour `HH:MM:SS`.

</details>

<details>
<summary>What happens with impossible dates like Feb 30?</summary>

They're validated against the real calendar and silently skipped — `Feb 30`,
`13/13/2024` and clock values like `25:99` never appear in the results, so
you don't get false positives from version numbers or scores.

</details>

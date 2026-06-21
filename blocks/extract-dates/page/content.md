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

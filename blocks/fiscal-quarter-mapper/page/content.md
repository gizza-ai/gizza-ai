## Map a whole CSV date column to fiscal quarters

Paste a CSV, TSV, semicolon, or pipe-delimited table that has a date column and get the same table back with `fiscal_quarter` and `fiscal_year` columns appended. The fiscal year can start in any of the twelve months — January for a calendar year, April for the UK and India, July for Australia and New Zealand, October for the US federal government — so the same export can be re-labelled for whichever calendar your reporting uses.

Unlike a single-date calculator this runs over the entire column, and unlike a spreadsheet formula it does not need a hand-built twelve-row lookup table. Everything runs locally in your browser: the table you paste is never uploaded.

### Worked example

Input, with **Date column** set to `closed` and **Fiscal year starts in** set to `October (US federal)`:

```csv
invoice,closed,amount
A-1001,2025-10-14,4200
A-1002,2026-01-07,1875
A-1003,2026-04-25,3060
```

Output:

```csv
invoice,closed,amount,fiscal_quarter,fiscal_year
A-1001,2025-10-14,4200,Q1 FY2026,FY2026
A-1002,2026-01-07,1875,Q2 FY2026,FY2026
A-1003,2026-04-25,3060,Q3 FY2026,FY2026
```

October 2025 opens the fiscal year that closes in September 2026, and the US federal convention names that year by the calendar year it **ends** in — so `2025-10-14` is Q1 **FY2026**, not FY2025. Switch **Fiscal year is named by** to *the year it begins in* and the exact same row becomes `Q1 FY2025`. This is the single most common wrong answer in fiscal-quarter work, so the tool makes it a deliberate choice rather than a silent default.

### Which calendar year names a fiscal year?

| Convention | Used by | `2025-10-14` with an October start |
|---|---|---|
| Named by the year it **ends** in (default) | US federal government, `pandas` `Period.qyear` | `Q1 FY2026` |
| Named by the year it **begins** in | The common spreadsheet fiscal-year formula | `Q1 FY2025` |

Both are genuinely in use and they always disagree by one. Pick the one your finance team already writes on its reports.

### Optional columns

- **Add fiscal_year column** — the fiscal-year label on its own, in the format you pick (`FY2026`, `2026`, `FY26`, `2025-2026`, `2025-26`). Range formats collapse to a single year when the fiscal year is the calendar year.
- **Add quarter start + end dates** — `fiscal_quarter_start` and `fiscal_quarter_end` as ISO dates, e.g. `2026-04-01` and `2026-06-30` for an October-start Q3.
- **Add fiscal month (1-12)** — the month's position counted from the fiscal start month, so with an October start, October is fiscal month `1` and April is `7`.
- **Add day of quarter / days in quarter** — `day_of_quarter` and `days_in_quarter` measured from each row's own date, e.g. `25` of `91`. Because it is measured per row rather than against today's date, re-running the tool on an old export always gives the same answer.

### Dates it reads

ISO `2026-04-25`, slashed `25/04/2026` and `04/25/2026`, dotted `25.04.2026`, two-digit years (`68` and below map to 2000s, `69` and above to 1900s), written months (`15 Jan 2024`, `Mar 3, 2024`, `October 2024`), compact `20240715`, month precision (`2024-11`, which maps from the 1st), and timestamps such as `2024-01-15T10:30:00Z` or `Mon, 15 Jan 2024 10:30:00 +0000`, whose clock part is discarded.

An all-numeric value like `03/04/2024` is genuinely ambiguous, so the column is read as a whole first: any row that can only be one thing — a leading number above 12, such as `25/12/2024` — settles day-first or month-first for every row. Choose **Output → Mapping report** to see which order was used and why.

### Limits and edge cases

- Input is capped at **5,000,000 bytes** (about 5 MB). For larger files use the CLI in a local pipeline.
- Impossible calendar dates such as `2021-02-30` are treated as unreadable, never rolled forward to 2 March.
- The report lists at most the **first 20** unreadable values, though the total count is always exact.
- Quarters are calendar quarters of the fiscal year and run **90 to 92 days**; 4-4-5 and 52/53-week retail calendars are a different scheme and are not supported here.
- Auto column detection needs at least **60%** of a column's non-blank cells to parse as dates; otherwise name the column explicitly by header or 0-based index.
- With **First row is a header** off, the date column must be given as a 0-based index, and no header row is written to the output.
- The output uses the same delimiter as the input, and existing columns are passed through untouched.

## FAQ

<details>
<summary>Is FY2026 the year that starts in 2026 or ends in 2026?</summary>

It depends on the convention, which is why this tool asks. The US federal government and `pandas` name a fiscal year by the calendar year it **ends** in, so the year running October 2025 to September 2026 is FY2026 — that is the default here. The widely-copied spreadsheet formula names it by the year it **begins** in, making the same period FY2025. Set **Fiscal year is named by** to match whichever your reports already use; the two answers always differ by exactly one.

</details>

<details>
<summary>Why do my fiscal quarters have different numbers of days?</summary>

Each fiscal quarter is three whole calendar months, and calendar months are not equal length. A quarter therefore runs 90, 91, or 92 days depending on which months it spans and whether it contains a leap day. Turn on **Add day of quarter / days in quarter** to see each quarter's actual length, for example 91 days for an October-start Q3 (1 April to 30 June).

</details>

<details>
<summary>How does the tool read `03/04/2024` — 3 April or 4 March?</summary>

It decides for the whole column rather than cell by cell. Every value is scanned first, and any row that can only be read one way settles the order for the rest: if the column also contains `25/12/2024`, day-first is proven and `03/04/2024` is 3 April. If nothing in the column proves either order, month-first is used. Force it with **Ambiguous numeric dates**, and check **Output → Mapping report** to see the order that was applied and the evidence for it.

</details>

<details>
<summary>What happens to a row whose date cannot be read?</summary>

That is what **Unreadable dates** controls. *Leave the new columns blank* keeps the row with empty fiscal columns, *Drop the row* removes it from the output, and *Stop with an error* halts and names the offending row number, line number, and value. In every mode the mapping report counts the unreadable rows and lists the first twenty offending values so you can fix the source data.

</details>

<details>
<summary>Which column does it use if I leave "Date column" on auto?</summary>

Auto picks the first column in which at least 60% of the non-blank cells parse as a date, so a table like `name,started` skips the names and maps `started`. If several columns hold dates — for example `created` and `closed` — auto takes the leftmost one, so name the column you want by its header, or by its 0-based index when the input has no header row.

</details>

<details>
<summary>Can it handle TSV, semicolons, or pipes?</summary>

Yes. The delimiter is sniffed from the first non-blank line by counting commas, tabs, semicolons, and pipes, and you can force a specific one with the **Delimiter** control. Whichever delimiter the input uses is also used for the output, so a TSV in gives a TSV out. Quoted fields containing the delimiter, such as `"New York, NY"`, are parsed and re-quoted correctly.

</details>

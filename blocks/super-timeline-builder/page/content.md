## About this tool

Build a single super-timeline from CSVs that have already been parsed from forensic artifacts. Paste
one table after another and introduce each table with a source header such as `--- mft ---`,
`=== evtx ===`, `==> prefetch <==`, or `# browser-history`. Each section keeps its own header row and
delimiter.

Worked example:

```csv
--- mft ---
Path,Created,LastModified
\Users\a\evil.exe,2024-06-01 10:00:05,2024-06-01 10:00:09
=== evtx ===
TimeCreated,EventID,Computer
2024-06-01 10:00:01,4624,DC01
```

With the default compact CSV output, the event-log row sorts first, followed by the MFT Created and
LastModified events. The default `expand=true` emits one event per timestamp column, which is useful
for file-system rows that carry created, modified, changed, and accessed times.

Limits and edge cases: this tool merges parsed CSVs only; it does not parse raw MFT, EVTX, browser
database, or registry hive files. Input is capped at 200,000 lines and output at the `limit` value
(max 100,000 rows). Timezone-less timestamps use `tz_offset`; timestamps that already contain `Z` or
an explicit offset keep their own offset and are normalized to UTC.

## FAQ

<details>
<summary>What should I paste into the input?</summary>

Paste CSV exports from tools that already parsed the artifact, such as an MFT listing, event-log CSV,
prefetch table, browser-history export, or registry report. Put a source header before each table so
rows can be traced back to the artifact that produced them.

</details>

<details>
<summary>Why does one input row become several timeline rows?</summary>

When `expand` is on, each timestamp column becomes its own event. For example, one file row with
Created and LastModified values becomes two timeline rows labelled with those column names. Turn
`expand` off if you only want the first detected timestamp column from each section.

</details>

<details>
<summary>Which output format should I choose?</summary>

Use `csv` for a compact `datetime,timestamp_desc,source,message` table. Use `l2tcsv` when you need the
legacy 17-field log2timeline-style layout for timeline viewers. Use `tln` for pipe-delimited
`Time|Source|Host|User|Description` rows with epoch-second timestamps.

</details>

<details>
<summary>How are timezones handled?</summary>

Timestamps with `Z` or an explicit `+/-hh:mm` offset are normalized directly to UTC. Naive timestamps
without a timezone use the `tz_offset` setting, so `tz_offset = -5` treats `2024-06-01 12:00:00` as
noon in UTC-05:00 and outputs `17:00:00Z`.

</details>

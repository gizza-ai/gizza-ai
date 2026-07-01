## About this tool

Convert a raw seconds value into common duration formats for video timestamps, logs, telemetry, schedules, and API payloads.

Supported formats:

- `hms` — `HH:MM:SS`, with hours accumulating past 24 (`90061` → `25:01:01`)
- `dhms` — `D:HH:MM:SS` (`90061` → `1:01:01:01`)
- `auto` — shortest clock form (`MM:SS`, `HH:MM:SS`, or `D:HH:MM:SS`)
- `iso` — ISO-8601 duration (`P1DT1H1M1S`)
- `words` — human-readable text (`1 day, 1 hour, 1 minute, 1 second`)

Seconds may be fractional or negative. Use the fractional digits field to retain decimal seconds after rounding. All conversion happens locally in your browser.

## FAQ

<details>
<summary>What happens when the duration is longer than 24 hours?</summary>

Depends on the format. `hms` lets the hours field grow past 24 — `90061` seconds
becomes `25:01:01` — which is what video editors and log tools usually expect.
If you'd rather see days split out, use `dhms` (`1:01:01:01`) or `auto`, which
picks the shortest sensible clock form.

</details>

<details>
<summary>How do I keep fractional seconds like 90.5?</summary>

Set the fractional digits field to 1–9. With `decimals=1`, `90.5` renders as
`00:01:30.5`; with the default of 0 the value is rounded to whole seconds. The
setting applies to every output format, including ISO durations.

</details>

<details>
<summary>Are negative durations supported?</summary>

Yes — a negative input is formatted like its positive counterpart with a leading
`-`, so `-3661` in `hms` gives `-01:01:01`. Handy for countdowns and clock-drift
deltas.

</details>

<details>
<summary>What does the ISO format output for zero?</summary>

`PT0S` — the canonical ISO-8601 rendering of a zero-length duration. Non-zero
values only include the units they need, e.g. `90061` → `P1DT1H1M1S`.

</details>

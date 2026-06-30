## About this tool

Convert a raw seconds value into common duration formats for video timestamps, logs, telemetry, schedules, and API payloads.

Supported formats:

- `hms` — `HH:MM:SS`, with hours accumulating past 24 (`90061` → `25:01:01`)
- `dhms` — `D:HH:MM:SS` (`90061` → `1:01:01:01`)
- `auto` — shortest clock form (`MM:SS`, `HH:MM:SS`, or `D:HH:MM:SS`)
- `iso` — ISO-8601 duration (`P1DT1H1M1S`)
- `words` — human-readable text (`1 day, 1 hour, 1 minute, 1 second`)

Seconds may be fractional or negative. Use the fractional digits field to retain decimal seconds after rounding. All conversion happens locally in your browser.

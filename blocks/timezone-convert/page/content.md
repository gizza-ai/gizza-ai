## Features
- **Smart Conversions**: Convert a date and time from a source timezone to multiple target zones instantly.
- **Visual Clocks**: Displays cards for each target showing times, dates, current offsets, and DST status.
- **Meeting Planner**: Automatically generates a 24-hour synchronized grid classifying times as *Business* (9 AM - 5 PM), *Leisure* (6 AM - 9 AM, 5 PM - 10 PM), or *Rest* (10 PM - 6 AM).
- **Interactive Tags**: Search and select target zones via autocomplete datalists and manage them as dismissible pills.
- **Offline & Private**: The tool runs entirely in your browser using local WebAssembly. No data is sent to a server.

---

## Timezone Reference

Select canonical IANA timezone names (e.g. `Area/Location`). The autocomplete fields suggest valid values as you type:

| Region | IANA Timezone | Standard/DST Offset |
|---|---|---|
| London | `Europe/London` | UTC+0 (GMT) / UTC+1 (BST) |
| New York | `America/New_York` | UTC-5 (EST) / UTC-4 (EDT) |
| Los Angeles | `America/Los_Angeles` | UTC-8 (PST) / UTC-7 (PDT) |
| Paris / Berlin | `Europe/Paris` | UTC+1 (CET) / UTC+2 (CEST) |
| Tokyo | `Asia/Tokyo` | UTC+9 (JST, no DST) |
| Mumbai | `Asia/Kolkata` | UTC+5:30 (IST, no DST) |
| Sydney | `Australia/Sydney` | UTC+10 (AEST) / UTC+11 (AEDT) |
| Universal Time | `UTC` | UTC+0 (no DST) |

*Note: Half-hour and 45-minute zones (e.g. India's UTC+5:30 or Nepal's UTC+5:45) are fully supported.*

---

## FAQ

<details>
<summary>How does it handle Daylight Saving Time (DST)?</summary>
<p>The tool bundles the complete IANA database, meaning historical, current, and future DST transitions are resolved accurately based on the date specified.</p>
</details>

<details>
<summary>What happens on spring-forward gap times?</summary>
<p>If a selected wall-clock time does not exist in the source zone due to a DST forward transition, the tool warns you that the time is non-existent instead of silently guessing a wrong value.</p>
</details>

<details>
<summary>Is my data private?</summary>
<p>Absolutely. All date, time, and timezone conversions run locally in your browser. No queries are transmitted to external servers.</p>
</details>

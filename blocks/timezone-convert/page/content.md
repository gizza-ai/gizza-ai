## What this tool does

Convert a date and time from one timezone to another (or multiple), right in your browser.
Enter a wall-clock time, pick the **From** zone, and specify one or more **To** zones (comma-separated). You will instantly get:
* The equivalent times in all target zones with their current UTC offsets.
* The difference in hours and minutes relative to the source time.
* Target weekdays and daylight saving time (DST) statuses.
* An interactive 24-hour **Meeting Planner Grid** showing overlapping hours classified as **Business** (9 AM - 5 PM), **Leisure** (6 AM - 9 AM, 5 PM - 10 PM), or **Rest** (10 PM - 6 AM).

Nothing is sent to a server; the tool runs locally in your browser, works offline, and requires no sign-up.

---

## Flexible Date & Time Input Formats

The tool features a lenient parser that accepts several common date and time formats. You do not need to restrict yourself to strict ISO format, nor should you specify a trailing `Z` or offset (the timezone is defined by the **From** field).

| Format Category | Example Input | Interpreted As |
| --- | --- | --- |
| **Standard ISO** | `2024-03-10 14:30` | 2:30 PM on March 10, 2024 |
| **ISO with seconds** | `2024-03-10T14:30:00` | Same, with seconds |
| **Date Slashes** | `2024/03/10 14:30` | Same, with slash separators |
| **AM/PM notation** | `2024-03-10 2:30 PM` | 12-hour clock with space |
| **AM/PM compact** | `2024-03-10 2:30PM` | 12-hour clock without space |
| **Date-only** | `2024-03-10` | Midnight (00:00) on that date |

---

## Dynamic Multi-Zone Calculations

To convert to multiple time zones at once, simply enter them in the **To timezone** field separated by commas. For example:
`Asia/Tokyo, Europe/London, UTC`

The tool will render a target clock card for each timezone, and populate a multi-column **Meeting Planner Grid** for the entire day, making it easy to schedule cross-border webinars, team syncs, and client calls.

---

## Daylight Saving Time is Handled for You

The conversion uses the full **IANA timezone database**, so daylight-saving (DST) rules are baked in. Convert a time on the day a region "springs forward" or "falls back" and the offset change is applied correctly. A time that falls in a spring-forward **gap** (the hour the clock skips, which never actually happens) is reported as non-existent rather than silently guessed.

---

## Timezone Names

Use canonical **IANA** names — `Area/Location`, for example:

| Region | IANA name | Typical Offset |
| --- | --- | --- |
| New York | `America/New_York` | UTC−5 (EST) / UTC−4 (EDT) |
| Los Angeles | `America/Los_Angeles` | UTC−8 (PST) / UTC−7 (PDT) |
| London | `Europe/London` | UTC+0 (GMT) / UTC+1 (BST) |
| Paris / Berlin | `Europe/Paris` | UTC+1 (CET) / UTC+2 (CEST) |
| Mumbai | `Asia/Kolkata` | UTC+5:30 (IST, no DST) |
| Tokyo | `Asia/Tokyo` | UTC+9 (JST, no DST) |
| Sydney | `Australia/Sydney` | UTC+10 (AEST) / UTC+11 (AEDT) |
| Universal Time | `UTC` | UTC+0 (no DST) |

Half-hour and 45-minute zones (like Nepal's UTC+5:45 or India's UTC+5:30) are fully supported.

---

## FAQ

**Is it free and private?**
Yes — your input never leaves your device, and it keeps working offline once the page has loaded.

**Why no `Z` or offset in the time input?**
So the meaning is unambiguous: the time is interpreted in the **From** zone you choose. Mixing an embedded offset with a zone name could conflict, so the tool asks for a plain wall-clock time instead.

**What if the time doesn't exist?**
On the spring-forward day, clocks jump (e.g. 2:00 → 3:00 AM), so times in that gap never occur. The tool tells you the time is non-existent instead of guessing.

**Does it know about historical DST rule changes?**
It uses the bundled IANA database, which encodes historical and current rules for each zone.

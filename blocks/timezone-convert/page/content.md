## What this tool does

Convert a date and time from one timezone to another, right in your browser.
Enter a wall-clock time, pick the **From** and **To** zones (by their IANA
names), and get the equivalent time in the target zone — with the UTC offsets,
the difference in hours, the target weekday, whether the result lands in daylight
saving time, and the Unix timestamp. Nothing is sent to a server; it runs
locally, works offline, and needs no sign-up.

## Daylight saving time is handled for you

The conversion uses the full **IANA timezone database**, so daylight-saving (DST)
rules are baked in. Convert a time on the day a region "springs forward" or
"falls back" and the offset change is applied correctly. A time that falls in a
spring-forward **gap** (the hour the clock skips, which never actually happens)
is reported as non-existent rather than silently guessed.

## How to enter the time

Give the date/time as a plain wall-clock value in ISO form — **do not** add a
`Z` or a `+02:00` offset, because the source zone is taken from the **From**
field, not the string.

| You type | Meaning |
| --- | --- |
| `2024-03-10 14:30` | 2:30 PM on 10 Mar 2024 |
| `2024-03-10T14:30:00` | same, with seconds |
| `2024-03-10` | midnight (00:00) on that date |

## Timezone names

Use canonical **IANA** names — `Area/Location`, for example:

| Region | IANA name |
| --- | --- |
| New York | `America/New_York` |
| Los Angeles | `America/Los_Angeles` |
| London | `Europe/London` |
| Paris / Berlin | `Europe/Paris` |
| Mumbai (UTC+5:30) | `Asia/Kolkata` |
| Tokyo | `Asia/Tokyo` |
| Sydney | `Australia/Sydney` |
| Coordinated Universal Time | `UTC` |

Half-hour and 45-minute zones (like India's UTC+5:30) are supported.

## Example

Convert **10 Jan 2024 14:30** in New York to Tokyo:

- From `America/New_York` (UTC−05:00) → To `Asia/Tokyo` (UTC+09:00)
- Result: **2024-01-11T04:30:00+09:00** — Tokyo is 14 hours ahead, so it is
  already the next morning.

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**Why no `Z` or offset in the time?** So the meaning is unambiguous: the time is
interpreted in the **From** zone you choose. Mixing an embedded offset with a
zone name could conflict, so the tool asks for a plain wall-clock time instead.

**What if the time doesn't exist?** On the spring-forward day, clocks jump (e.g.
2:00 → 3:00 AM), so times in that gap never occur. The tool tells you the time is
non-existent instead of guessing.

**Does it know about historical DST rule changes?** It uses the bundled IANA
database, which encodes historical and current rules for each zone.

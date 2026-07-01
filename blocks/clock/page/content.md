## About this clock

This page shows the current UTC (Coordinated Universal Time) date and time,
updating every second. The time is read from your own device's clock — nothing
is sent to a server, and it works offline.

### What is UTC?

UTC is the global time standard that clocks and time zones are based on. Unlike
local time, it does not change with daylight saving.

### FAQ

<details>
<summary>Why UTC?</summary>

It is unambiguous worldwide — useful for logs, scheduling across
time zones, and coordination.

</details>

<details>
<summary>Is it accurate?</summary>

It is as accurate as your device's clock.

</details>

<details>
<summary>What format is the timestamp in?</summary>

ISO 8601 / RFC 3339 — for example `2026-04-20T12:00:00+00:00`, where `+00:00`
marks UTC. This format sorts chronologically as plain text, which is why it's
the standard for logs and APIs.

</details>

<details>
<summary>How do I get my local time from the UTC reading?</summary>

Add (or subtract) your time zone's UTC offset — e.g. New York is UTC−5 in
winter and UTC−4 in summer. UTC itself never observes daylight saving, so the
offset you apply is what changes, not the UTC reading.

</details>

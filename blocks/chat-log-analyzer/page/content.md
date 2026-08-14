## About this tool

Paste a chat transcript or IRC log to get a local, deterministic activity report. The analyzer counts messages, participants, words, characters, links, channel events, `/me` actions, busiest hours, and weekday activity without uploading the log anywhere.

It accepts common line shapes such as `21:07 <alice> hey`, `[21:07:33] <@alice> hey`, `2024-01-05 21:07:33 <alice> hey`, tab-separated WeeChat exports, and plain `alice: hey` transcripts. Use the bot exclusion field for service accounts such as `gizzabot` or wildcard prefixes such as `travis*`.

Example input:

```text
2024-01-05 21:07:33 <alice> pizza tonight? https://example.com/menu
2024-01-05 21:08:01 <@bob> pizza sounds great
2024-01-05 21:09:00 -!- carol [~c@host] has joined #food
2024-01-06 09:16:10 * alice waves
```

Example summary output includes:

```text
Messages: 3
Participants: 2
Busiest hour: 21:00
Who talked most
Top words
Links shared
Channel events
```

Limits and edge cases: the input is capped at 5,000,000 bytes; one pasted log is analyzed at a time; event-only logs return an error because there are no messages to count; weekday stats need dated lines; time-only logs still produce hour stats; ambiguous numeric dates default to day-first unless the data proves month-first; stopword filtering is English-only.

## FAQ

<details>
<summary>Which chat log formats does it understand?</summary>

It auto-detects common IRC-style formats with angle-bracket nicks, optional bare or bracketed timestamps, ISO dates, 12-hour AM/PM times, tab-separated WeeChat columns, `/me` actions, and simple `nick: message` transcripts. You do not need to choose a format first.

</details>

<details>
<summary>Do joins, parts, quits, and mode changes count as messages?</summary>

No. Channel events are counted in the events section, but they are excluded from message, word, link, and participant rankings. `/me` action lines are counted as messages and also listed separately as actions.

</details>

<details>
<summary>How do I remove bot noise from the results?</summary>

Add bot nicks to the exclude field. A comma-separated list such as `gizzabot, travis*` removes the exact nick `gizzabot` and every nick beginning with `travis` from all statistics.

</details>

<details>
<summary>Why is the weekday section empty for my log?</summary>

Weekday activity needs full dates. Logs with only times, such as `21:07 <alice> hi`, can still report busiest hours and participant rankings, but the analyzer cannot infer the day of week without a date.

</details>

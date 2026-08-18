## About this tool

YouTube Takeout Stats turns a Google/YouTube Takeout watch-history export into a local, copy-pasteable viewing report. Paste either `watch-history.json` or `watch-history.html` and it summarizes total watches, unique videos, unique channels, top channels, repeated videos, monthly trends, weekday patterns, hourly patterns, busiest day, and longest active streak.

The default output is a text dashboard. Switch to CSV for a single table such as top channels or videos per month, or JSON when you want structured data for another script. JSON exports use UTC timestamps, so set the UTC offset to your local time before bucketing by day and hour; HTML exports already contain local wall-clock timestamps and do not need the offset.

### Worked example

Paste this minimal Takeout-style JSON and run the default dashboard:

```json
[{"header":"YouTube","title":"Watched Never Gonna Give You Up","titleUrl":"https://www.youtube.com/watch?v=dQw4w9WgXcQ","subtitles":[{"name":"Rick Astley"}],"time":"2024-01-01T18:10:00Z"},{"header":"YouTube","title":"Watched Never Gonna Give You Up","titleUrl":"https://www.youtube.com/watch?v=dQw4w9WgXcQ","subtitles":[{"name":"Rick Astley"}],"time":"2024-01-02T19:00:00Z"},{"header":"YouTube","title":"Watched Rust in 100 Seconds","titleUrl":"https://www.youtube.com/watch?v=5C_HPTJg5ek","subtitles":[{"name":"Fireship"}],"time":"2024-01-03T09:05:00Z"}]
```

The result starts with `YouTube watch history — 3 videos watched`, then lists the range, averages, unique video/channel counts, peak hour, longest streak, top channels, top videos, and month/weekday/hour tables.

### Limits and edge cases

- Input stays local in the browser. The WebAssembly code parses the pasted text without logging in, uploading the file, or calling the YouTube API.
- The tool accepts up to 24 MB of pasted text and up to 500,000 parsed watch records.
- If your Takeout came as a ZIP archive, unzip it first and paste the `watch-history.json` or `watch-history.html` file.
- The Takeout watch-history export does not include reliable video durations or categories. Those require authenticated YouTube Data API lookups, so this tool reports counts and timing patterns only.
- Ads and YouTube Music rows are excluded by default because they often distort regular viewing stats. Turn on the checkboxes when you want those rows included.
- Removed or private videos are counted, but they may not have a channel name in the export.

## FAQ

<details>
<summary>Does this upload my YouTube history anywhere?</summary>

No. The parser runs in WebAssembly inside your browser and the CLI runs locally. The tool does not call YouTube, does not require an account, and does not upload the pasted export.

</details>

<details>
<summary>Which Takeout file should I paste?</summary>

Use `YouTube and YouTube Music/history/watch-history.json` when you exported Takeout as JSON. If your export is HTML, paste the contents of `watch-history.html`; this tool supports both shapes.

</details>

<details>
<summary>Why are watch minutes and video categories missing?</summary>

Google Takeout watch history records the activity item and timestamp, but it does not include the duration watched or the category for each video. Getting those fields would require live YouTube API requests and an API key, which is outside this offline tool's model.

</details>

<details>
<summary>How should I use the UTC offset field?</summary>

For JSON exports, timestamps are UTC. Set the offset to your local time zone, for example `-5` for UTC-5 or `5.5` for UTC+5:30, so days and peak hours match your local viewing time. HTML exports already contain local text timestamps, so the offset is ignored for them.

</details>

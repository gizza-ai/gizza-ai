# youtube-takeout-stats — competitor analysis (2026-08-18)

Scan run **before** implementing, per the create-next-tool recipe. One web search
("YouTube watch history Takeout analyzer stats top channels videos per month tool")
plus direct reads of the top hits. All notes below are **paraphrased observations** —
no competitor copy, wording, branding, or asset is reproduced or reused anywhere in
this tool.

## Competitors reviewed

| # | Tool | Shape | Reachable |
|---|------|-------|-----------|
| 1 | Playback Stats (playbackstats.com) | Browser-local dashboard, upload Takeout ZIP or `watch-history.json` | yes |
| 2 | positron48/youtube-history-analyzer (GitHub) | Python CLI → CSV/HTML/JSON reports | yes |
| 3 | luciopaiva/youtube-takeout-analyzer (GitHub) | Node scripts → console + CSV reports | yes |
| — | ajay.app "Watch History Statistic Viewer" | reachable but the page carries almost no documented detail (feature bullets only), so it was **replaced** by #3 as the third profiled tool. Its three documented outputs (top videos, top channels, top categories) are folded into the table below. |

## What they accept

- **Playback Stats:** a Google Takeout **ZIP** or a raw `watch-history.json`, with a
  stated ~100 MB ceiling on the JSON. No HTML variant documented.
- **positron48:** `watch-history.json` from the YouTube/YouTube-Music export, and
  `MyActivity.json` for pre-2021 history. Dedupes on video id + timestamp.
- **luciopaiva:** the Takeout watch-history file plus the Watch Later playlist
  export; also fetches durations from the YouTube Data API (quota-limited).

Nobody in the set documents reading the **HTML** variant (`watch-history.html`) that
Takeout still hands out when the export format is left on HTML — that is a real,
in-model gap we can close.

## Table-stakes capabilities (from the three profiles)

| Capability | Seen in | Our decision |
|---|---|---|
| Total videos watched | 1, 2 | **in-model — built** (overview) |
| Unique videos / unique channels | 1, 2 | **in-model — built** |
| Date range covered (first → last watch) | 1, 2 | **in-model — built** |
| Active days + average videos per day | 2 | **in-model — built** (per active day *and* per calendar day) |
| Top channels by view count | 1, 2, 3, ajay | **in-model — built** (`report=channels`, `top` cap) |
| Top / most-repeated videos | 1, 3, ajay | **in-model — built** (`report=videos`) |
| Videos per month (trend) | 1, 2, 3 | **in-model — built** (`report=months`, gap months filled with 0) |
| Hour-of-day distribution + peak hour | 1, 2 | **in-model — built** (`report=hours`) |
| Weekday distribution + favourite weekday | 1, 2 | **in-model — built** (`report=weekdays`) |
| Longest active streak | 1 | **in-model — built** |
| Busiest single day | 1 | **in-model — built** |
| Concentration (share of views held by the top channel) | 1 | **in-model — built** (per-channel share %) |
| Exclude YouTube Music rows | 2 (hard-coded) | **in-model — built, as a toggle** (`include_music`, default off) |
| Exclude ad impressions | — (none handle it explicitly) | **in-model — built** (`include_ads`, default off) |
| CSV export of a table | 2, 3 | **in-model — built** (`output=csv` for every report) |
| JSON export of the summary | 2 | **in-model — built** (`output=json`) |
| Date-range filter | none | **in-model — built** (`start_date` / `end_date`, native date pickers) — a genuine gap in all three |
| Reads `watch-history.html` | none | **in-model — built** — a genuine gap in all three |
| Minutes watched / total watch time | 2 (via API), 3 (via API) | **out-of-model.** Durations are not in the Takeout export; both competitors call the YouTube Data API with an API key and a daily quota. gizza tools are browser-local, keyless and offline, so this is listed, not built. Playback Stats reaches the same conclusion and says outright that reliable watch minutes are not derivable from the export. |
| Video categories | ajay | **out-of-model.** Category is not in the export either; it needs the same authenticated Data API lookup. |
| Reading the Takeout **ZIP** directly | 1 | **out-of-model for this tool.** The page/CLI surface takes a text field, not an archive; the repo already ships a separate unzip tool, so the documented flow is unzip → paste `watch-history.json`. Listed on the page. |
| Interactive Plotly-style charts | 2 | **considered, rejected.** The generator renders text/CSV/JSON output; a chart renderer would be per-tool custom JS. The month/weekday/hour tables carry inline ASCII bars instead, which stay copy-pasteable. |
| Watch Later playlist analysis | 3 | **out-of-scope** — a different export file, not watch history. |
| Fetching history live from a YouTube account | ajay ("syncs with YouTube") | **out-of-model** — needs an account/OAuth; gizza is no-account. |

## UX controls worth matching

- Competitors are upload-and-see-a-dashboard: **zero configuration**. Our defaults are
  therefore chosen so that pasting the file alone produces the full dashboard
  (`output=text`, `report=overview`, `top=10`, ads and Music excluded).
- Playback Stats leads with a small set of "headline" numbers (peak hour, favourite
  weekday, longest streak, busiest day) before the long tables. Our text overview
  follows the same information ordering: totals → range → averages → habits →
  top channels → top videos → month/weekday/hour tables.
- Presets: none of the three ship preset buttons, but the report/format matrix
  benefits from them, so the page carries `[[example]]` chips (full dashboard,
  top channels CSV, monthly trend, JSON summary) that prefill and run in one click.
- Privacy is the headline claim for the two browser tools. Ours is stronger and is
  stated plainly on the page: the parse is WebAssembly running locally, with no
  upload and no network request.

## Sources

- https://playbackstats.com/
- https://github.com/positron48/youtube-history-analyzer
- https://github.com/luciopaiva/youtube-takeout-analyzer
- https://ajay.app/YouTube-Watch-History-Statistic-Viewer/ (thin; replaced by luciopaiva)

# log-timestamp-normalizer — competitor analysis (2026-08-14)

Scan done **before** implementation so the descriptor could be designed against the table-stakes
rather than retrofitted. Everything below is **paraphrased** from public product pages and docs —
no competitor copy, branding, or trademarks are reproduced or reused.

## Sources scanned

| # | Competitor | Shape | What it is |
|---|-----------|-------|------------|
| 1 | CompuTools "Log Timestamp Parser & Sorter" | browser tool | Detects ISO-8601, epoch s/ms and RFC 2822 in pasted log lines, reformats them and can sort the lines chronologically; client-side only. |
| 2 | WalkAgain Tools "Log Timestamp Normalizer" | browser tool | Auto-detects ISO 8601, 13-digit epoch ms, 10-digit epoch s and bracketed/slashed date-time forms; converts to a chosen output format and timezone; optional keep-the-whole-line mode. |
| 3 | Apify "Timestamp Converter" (bulk) | hosted actor | Bulk conversion of a mixed list of epoch seconds/ms, ISO 8601 and natural-language dates; auto-detects each entry's format; emits UTC. |
| 4 | Tiny-Online.Tools "ISO Date Converter" | browser tool | Converts ISO strings, RFC 2822 / HTTP dates and epoch s/ms to ISO-8601 UTC; epoch unit inferred from digit count. |
| 5 | Microsoft WinGet Log Viewer (VS Code) | log viewer | Viewer-side "show time delta" mode that annotates each visible line with the elapsed time since the previous one (`+Xms` / `+Xs`) to make slow steps visible. |
| — | maketimestamp.com "Timestamps in logs" guide | reference doc | Enumerates the formats that actually show up in logs (ISO-8601/RFC 3339, Apache/nginx bracketed, syslog RFC 3164 without a year, epoch at s/ms/µs/ns precision) and the normalization traps: missing zone, missing year, mixed precision, DST. |

Also observed in the broader category (LogViewPlus, Acacia Log Viewer, Splunk `delta`, Grafana Loki):
delta-between-consecutive-events is a first-class feature in professional viewers, usually as a
computed column plus a "highlight gaps over N" affordance.

## Table-stakes extracted → where each one landed

| Table-stake | Seen at | Decision |
|---|---|---|
| Auto-detect the timestamp format per line (no manual "input format" step) | 1, 2, 3, 4 | **In model — built.** Detection is per line, so one paste can mix formats. |
| ISO-8601 / RFC 3339 input, with or without fractional seconds, `T` or space separator | 1, 2, 3, 4 | **Built.** |
| Epoch seconds (10 digits) and milliseconds (13 digits) | 1, 2, 3, 4 | **Built**, plus microseconds (16) and nanoseconds (19) and fractional epoch (`1701425730.123`) from the maketimestamp guide. |
| Apache/nginx bracketed date (`10/Oct/2000:13:55:36 -0700`) | 6 | **Built.** |
| Syslog RFC 3164 (`Dec  1 10:15:30`, no year, no zone) | 6 | **Built**, with an explicit `assume_year` control — see "year gap" below. |
| RFC 2822 / HTTP date (`Sat, 01 Dec 2024 10:15:30 +0000`) | 1, 4 | **Built** (input and output). |
| `YYYY-MM-DD HH:MM:SS` and `YYYY/MM/DD HH:MM:SS`, bracketed or bare | 1, 2 | **Built.** |
| Choice of output format (ISO-8601, ISO with ms, epoch s, epoch ms, plain date-time) | 1, 2 | **Built** as `output_format`, plus RFC 2822 out. |
| Target timezone for the output (UTC default, local, a named zone) | 2 | **Built** as `output_timezone`, accepting `UTC`, an IANA zone name (DST-correct via the bundled tz database) or a fixed offset. "Local" is the page's job — the field autocompletes from the timezone vocabulary. |
| Sort the normalized lines (none / oldest first / newest first) | 1, 2 | **Built** as `sort`, default `input` (leave order alone) so the tool is non-destructive by default. |
| Keep the original log line vs. emit timestamps only | 2 | **Built** as `output_mode`, with a third `prefix` mode (normalized stamp prepended, original line untouched) that neither tool offers. |
| Detection statistics panel ("how many lines matched what") | 1 | **Built** as the optional `summary` header — counts per detected format, the time span, and the largest gap, as `#` comment lines so the output is still pasteable. |
| Delta between consecutive lines, rendered compactly (`+1.2s`) | 5, and the viewer category | **Built** as `delta` (on by default — it is the second half of this tool's job) with `delta_format` = auto / seconds / milliseconds / `h:mm:ss`. |
| Highlight long gaps between events | 5, LogViewPlus, Acacia | **Built** as `gap_threshold_seconds` (0 = off) which appends a `GAP` marker to any delta at or above the threshold. |
| A one-click "load example" so the tool shows output before you type | 1 | **Built** as four `[[example]]` preset chips (mixed formats, syslog with a year, epoch to date-time, gap hunting). |
| Explicit handling of timestamps with no zone | 6 | **Built** as `assume_timezone` — zone-less stamps are interpreted in that zone (DST-correct for named zones) instead of being silently treated as UTC. |
| Explicit handling of syslog's missing year | 6 | **Built** as `assume_year`; `0` (default) infers the year from the nearest line in the same paste that carries one, and only falls back to leaving the line unmatched if the paste has no dated line at all. |
| Lines with no timestamp at all (stack traces, banners) must survive | 1, 2 | **Built** as `unmatched` = keep / drop / mark. |

Nothing from the scan was dropped silently: every row above is either in the descriptor or in the
out-of-model list below.

## Out of model (listed, deliberately not built)

- **Custom regex input pattern + strftime output pattern** (competitor 1). Buildable in principle,
  but a free-form regex box on a browser tool is a footgun (catastrophic backtracking, silent
  zero-match) and the auto-detector already covers the formats the maketimestamp survey found in
  real logs. Reconsider if a concrete unmatched format shows up.
- **Upload/stream a whole log file, or bulk-convert a dataset server-side** (competitor 3 is a
  hosted actor with storage + an API). This tool is browser-local with no backend; the paste box is
  capped at 50,000 lines.
- **Live tailing, remote log sources, SQL over the parsed table** (LogViewPlus, Loki, Splunk).
  Needs a server and a session; out of model for a stateless page.
- **Natural-language date input** ("yesterday 3pm", competitor 3). Depends on a locale-aware NLP
  date parser and a reference "now"; the tool is deterministic and clock-free by design so the same
  paste always produces the same bytes.
- **Ambiguous `MM/DD/YYYY` vs `DD/MM/YYYY` bare dates.** Not detected on purpose — guessing wrong
  silently shifts events by months. Documented as a limitation on the page.

## Considered, rejected

- **A "local time" enum choice** next to UTC. The zone field already accepts any IANA name and the
  page autocompletes them; adding a magic `local` value would make the CLI's output depend on the
  machine's clock configuration and break reproducibility.
- **Sorting on by default.** Competitors default to no sorting too, and re-ordering someone's log
  without being asked destroys the interleaving they were reading.

## Our differentiators after this build

- Per-line auto-detection across **eight** input shapes in one paste, including µs/ns epoch and
  syslog-with-inferred-year, which none of the five scanned tools covers together.
- The delta annotation and the format normalization happen in **one pass** — the scanned tools do
  one or the other, never both.
- DST-correct named-zone handling on both sides (`assume_timezone` for input, `output_timezone` for
  output) via a bundled IANA database, with no network call.
- Runs entirely in the browser (and in the CLI) with no upload, no account, and no request quota.

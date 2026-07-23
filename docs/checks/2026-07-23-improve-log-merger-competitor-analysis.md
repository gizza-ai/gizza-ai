# log-merger — competitor analysis (2026-07-23)

Dup check: `blocks/log-analyzer` (aggregate summary of ONE log), `blocks/log-parser`
(row-by-row table/CSV/JSON of ONE log), `blocks/ip-log-anonymizer` (IP masking). None
of these **interleave multiple sources into one timeline** — `log-merger` is a distinct
function (merge N sources, sort by parsed timestamp, tag each line with its source). Not
a semantic duplicate → viable.

One web search ("merge multiple log files by timestamp online tool interleave logs
unified timeline"). Findings paraphrased below — no copy/branding reused.

## Competitor landscape (top real tools)

| tool | what it does well | dimension | in-model? |
| ---- | ----------------- | --------- | --------- |
| ptmcg **logmerger** (Python TUI) | reads N files, merges by timestamp into an interactive terminal timeline; can save merged output; understands several timestamp shapes; treats untimestamped lines as continuations of the previous entry | capabilities/UX | continuation carry-forward → in-model; interactive TUI → out-of-model (browser page instead) |
| jamesbattersby **vscode-loginterleaver** | interleaves multiple open files by timestamp; multiple regexes to extract timestamps from differing formats; combines into one ordered buffer | capabilities | multi-format auto timestamp parsing → in-model |
| hackitu.de **Logfile merger** | reads several logfiles in parallel, evaluates each line's timestamp, emits one time-sorted stream | capabilities | core merge/sort → in-model |
| PyPI **log-merger** (CLI) | command-line merge-by-timestamp of multiple files | capabilities | CLI merge → in-model (we also ship a CLI) |
| generic `sort -m` / `logmerge` scripts | merge already-sorted streams by leading timestamp | capabilities | leading-timestamp parse → in-model |

## Gap list → decisions

- **Multi-format timestamp auto-detection** (ISO 8601/RFC 3339, `YYYY-MM-DD HH:MM:SS`,
  syslog `Mon DD HH:MM:SS`, Apache `10/Oct/2000:13:55:36 -0700`, unix epoch s/ms) — IN.
  Parsed from anywhere in the line, not just the start.
- **Continuation lines inherit the previous line's timestamp** (stack traces / wrapped
  messages stay attached to their entry) — IN (carry-forward).
- **Source tagging** — every merged line prefixed `[source]`; source names come from
  header lines (`--- app.log ---`, `=== name ===`, GNU `tail`'s `==> name <==`, or
  markdown `# name`). IN.
- **Aligned source tags** (pad `[tag]` to a common width so messages line up) — IN
  (`align`, our own UX nicety; competitors don't offer it).
- **Ascending / descending order** — IN (`order`).
- **De-duplicate identical timestamped lines** across overlapping captures — IN
  (`dedupe`).
- **Blank-line-separated source blocks** for quick unnamed pastes — IN (`source_mode=blank`).
- **Line-count cap + clear error, and a "no parseable timestamps" error** — IN.
- Interactive TUI / scrollback, live tail, per-file colour theming, reading files from
  disk/URLs — **out-of-model** (needs a terminal / filesystem / server); the browser page
  + CLI cover the paste-and-merge use case instead.

Positioning: the only browser-local, no-upload, no-install timeline merger — paste 2+
logs, get one sorted stream with source tags; the CLI covers scripted use.

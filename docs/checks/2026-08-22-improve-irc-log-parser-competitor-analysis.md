# irc-log-parser — competitor analysis (2026-08-22)

Scan run **before** implementation, per `/create-next-tool` step 4. One WebSearch
("IRC log parser viewer convert weechat irssi logs to structured JSON timeline tool") plus
direct reads of four real tools. Everything below is **paraphrased** from public docs — no
competitor copy, wording, branding or trademarks are reused anywhere in the tool.

## Tools skimmed

| # | Tool | What it is | Read |
|---|------|-----------|------|
| 1 | ilc (tilpner/ilc) | Rust CLI log converter/collector | README |
| 2 | irclog (irclog.readthedocs.io) | Python IRC log parser + WSGI web viewer | `irclog.parser` API docs |
| 3 | node-irssi-log-parser (kfranqueiro) | Node module + JSON export scripts for irssi logs | README |
| 4 | ircjournal (zopieux) | Standalone web log viewer with live ingestion | README |

## What they do (paraphrased)

**ilc** reads EnergyMech/ZNC and WeeChat logs (irssi listed as planned) and offers
`parse` / `convert` / `freq` / `seen` / `sort` / `dedup` subcommands plus binary and msgpack
representations. Options that matter to us: `--date` to override the log's date, `--infer-date`
to take the date from the filename, `--tz` as a UTC offset in seconds, `--channel` to attach a
channel name to a log that doesn't carry one, and explicit `--inf` / `--outf` format selection
rather than pure guessing.

**irclog** models a fixed set of record types — public message, action, notice, join, part,
quit, kick, nick change (including the logging user's own), mode, topic set and topic cleared —
and extracts named fields per record: timestamp, nick, channel, reason and message body. Its
`parse()` takes an optional base `date` (defaulting to today) because client logs frequently
carry a time only, and it assumes UTF-8 input over an iterable of lines.

**node-irssi-log-parser** emits one event per log line where every event carries at least a
`type` and a `time`, with the remaining fields varying by type. It ships two JSON exporters —
a full dump of parsed lines and an activity/statistics roll-up with `limit`, nickname
`synonyms` and `indent` options — and lets the caller override the built-in regular expressions
via a config file, i.e. format handling is explicitly pluggable.

**ircjournal** ingests WeeChat-format logs only and is a viewer rather than a converter: a
browsable per-channel timeline with full-text search, regexp search in the page, permalinks to
a single line or a block of lines, optional live tailing, and a client-side toggle that hides
join/part noise.

## Table stakes → in-model / out-of-model

| # | Table stake (paraphrased) | Seen in | Decision |
|---|---------------------------|---------|----------|
| 1 | Read the common client dialects, not one | ilc, pyirclogs, irclog | **In** — `format` enum: `auto`, `weechat` (date TAB nick TAB text), `irssi` (bare `HH:MM`), `bracket` (`[21:07:33]` — mIRC / ZNC / EnergyMech), `hexchat` (`Jan 05 21:07:33`), `iso` (`2024-01-05 21:07:33`), `plain` (no timestamps) |
| 2 | Explicit format selection, not guess-only | ilc `--inf` | **In** — `format` defaults to `auto` but can be pinned; auto-detection scores every dialect over the first 200 non-blank lines |
| 3 | A typed event model, not just text | irclog, node-irssi-log-parser | **In** — 12 kinds: `message`, `action`, `notice`, `join`, `part`, `quit`, `kick`, `nick`, `mode`, `topic`, `meta`, `unknown` |
| 4 | Named fields per record | irclog, node-irssi-log-parser | **In** — fixed 8-column record: `line`, `time`, `type`, `nick`, `host`, `channel`, `arg`, `text` (+ `raw` on request) |
| 5 | Structured JSON export | node-irssi-log-parser, ilc | **In** — `output=json` (pretty array) and `output=ndjson` (one object per line, for `jq`/streaming) |
| 6 | Spreadsheet / table export | ilc convert | **In** — `output=csv` (RFC 4180 quoting) and `output=markdown` |
| 7 | A readable rendered timeline | ircjournal, irclog viewer | **In** — `output=timeline`, the default; normalizes every dialect to one `<nick> text` / `--> joined` / `<-- quit` rendering |
| 8 | Base-date override for time-only logs | ilc `--date`, irclog `date=` | **In** — `date` param (`YYYY-MM-DD`) |
| 9 | Pick the date up from the log itself | ilc `--infer-date` (filename) | **In, adapted** — filenames aren't available to a paste-in tool, so we read irssi's `--- Log opened …` and `--- Day changed …` markers instead and roll the date forward mid-log |
| 10 | Timestamp normalization | irclog, viewers | **In** — `time_format`: `iso`, `24h`, `12h`, `original`, `none` |
| 11 | Attach a channel to a log that lacks one | ilc `--channel` | **In** — `channel` param fills the channel field wherever the line didn't name one |
| 12 | Hide join/part/quit noise | ircjournal toggle | **In** — `include`: `all`, `messages` (message/action/notice), `events` (join/part/quit/kick/nick/mode/topic) |
| 13 | Filter to particular people | node-irssi-log-parser `synonyms`/`limit` (partially) | **In** — `nicks` comma list, case-insensitive, trailing `*` matches by prefix |
| 14 | Cap the output size | node-irssi-log-parser `limit` | **In** — `limit` 0–200000, applied after filtering |
| 15 | Keep the original line available | ilc dedup/convert round-trips | **In** — `include_raw` checkbox adds a `raw` field/column |
| 16 | Cope with formatting control codes | every viewer strips them to render | **In** — `strip_formatting` (default on) removes mIRC bold/italic/underline/reverse/reset and `^C` colour codes plus ANSI CSI escapes |
| 17 | Timezone shifting (`--tz`) | ilc | **Out (listed, not built)** — a client log records no source timezone, so a shift is a guess the user must supply per file, and it only pays off when merging logs from two machines; `blocks/log-merger` is the tool that already owns merge-time alignment. Revisit if a merge surface needs it. |
| 18 | Frequency / "seen" / activity statistics | ilc `freq`/`seen`, node-irssi export-activity-json | **Out (already ours)** — `blocks/chat-log-analyzer` already produces who-talked-most, per-hour/weekday activity, word and link stats for IRC logs. Duplicating it here would collide on discovery; this tool stays a parser/exporter. |
| 19 | Sort / dedup / merge across files | ilc | **Out (already ours)** — `blocks/log-merger` merges and orders multiple logs; this tool is single-paste. Records keep their source `line` number so downstream sorting is possible. |
| 20 | Binary / msgpack representations | ilc | **Out** — the page and CLI surfaces are text; JSON/NDJSON cover machine consumption. |
| 21 | Full-text server-side search, live tailing, permalinks, per-day browsing | ircjournal, irclog viewer | **Out of model** — those need a server that ingests and hosts log files. gizza tools are local, stateless, single-shot transforms with no storage. |
| 22 | User-supplied regex overrides for unknown dialects | node-irssi-log-parser `--config` | **Out** — a JSON-of-regexes parameter is a poor fit for a chat/CLI schema and an injection-shaped footgun; the seven built-in grammars plus `plain` cover the clients people actually export from, and unrecognized lines survive as `unknown` records rather than being dropped. |

## Deliberate non-overlaps with existing gizza blocks

- `blocks/chat-log-analyzer` — IRC-aware **statistics** (rankings, bar charts). This tool emits
  the parsed **records** instead; no counting, no ranking.
- `blocks/chat-transcript-formatter` — reformats generic chat (WhatsApp/Discord/`Name:` lines)
  into a human transcript. It has no IRC event model, no structured export, and deliberately
  reads `Name: message`; this tool does **not** treat `nick: text` as a message so the two
  don't compete on the same input.
- `blocks/log-parser`, `blocks/log-merger`, `blocks/ansi-log-renderer` — server/syslog/access
  logs, multi-file merging, and ANSI-to-HTML rendering respectively; no IRC grammar in any.

## UX patterns adopted

- Preset chips (`[[example]]`) for the four dialects most people paste: an irssi day, a WeeChat
  export to CSV, a mIRC/ZNC session to JSON, and a messages-only reading view.
- Friendly `<select>` labels (`[input.labels]`) naming the actual clients next to each grammar,
  because "bracket" means nothing to someone holding a mIRC log.
- A multiline paste field with a realistic irssi placeholder, matching how every competitor is
  fed (a pasted or piped log body).

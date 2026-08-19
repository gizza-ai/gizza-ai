# chat-log-analyzer — competitor analysis (2026-08-14)

Scan run **before** implementing, per `/create-next-tool` step 3. One WebSearch
("IRC chat log analyzer tool statistics who talked most activity by hour word stats"),
then three reachable competitor tools were skimmed. All notes below are **paraphrased
observations of capabilities**; no competitor copy, branding, or trademarks were used.

## Tools skimmed

| # | Tool | What it is | Reachable? |
|---|------|-----------|-----------|
| 1 | hisg (IRC statistics generator) | Static stats-page generator for irssi logs | yes (project wiki) |
| 2 | ConvoMetrics | Chat analytics dashboard for exported chats | yes (project README) |
| 3 | tomzx/irc-stats | PHP library for IRC activity analysis | yes (project README) |
| — | IRCReplay | Browser IRC log viewer + stats generator | **403 to the fetcher — replaced by #3** |
| — | pisg | Classic Perl IRC stats generator | project page redirected to a listing with no feature detail; used only as a general reference point, not counted in the three |

## Table-stakes observed (union across the three)

| # | Capability | Seen in | Verdict | Where it landed |
|---|-----------|---------|---------|-----------------|
| 1 | Per-nick message counts, ranked ("who talked most") | 1, 2, 3 | **in-model** | `Who talked most` table, ranked, `top`-capped |
| 2 | Per-nick word and character counts | 1, 2 | **in-model** | same table (Words, Chars columns) |
| 3 | Characters/words **per line** (verbosity comparison) | 1 | **in-model** | `Avg words` + `Avg chars` columns |
| 4 | Activity distribution by hour of day | 1, 2, 3 | **in-model** | `Activity by hour` ASCII bar histogram (24 rows collapsed to non-empty) |
| 5 | Activity by day of week | 2 | **in-model** | `Activity by weekday` histogram (needs dated lines) |
| 6 | Busiest day / busiest hour callout | 2 | **in-model** | Overview line |
| 7 | Total messages / words / characters / days talked | 2 | **in-model** | Overview line |
| 8 | Word frequency with stopword filtering | 1, 2, 3 | **in-model** | `Top words` + `ignore_stopwords` / `min_word_length` params |
| 9 | Time span of the log (first → last) | 2 | **in-model** | Overview line |
| 10 | Links/URLs shared | (IRCReplay listing; common in this class) | **in-model** | `Links shared` section — count + top domains |
| 11 | Excluding bots / specific nicks from stats | pisg-class convention | **in-model** | `exclude_nicks` param (comma list, `*` wildcard suffix) |
| 12 | Multiple client log dialects (irssi, mIRC, HexChat, WeeChat, ZNC) | 1, 3 (chatdb) | **in-model** | Auto-detecting parser, no format param needed |
| 13 | IRC events: joins/parts/quits/nick changes/kicks | 3 | **in-model** | `Events` section, and events excluded from message stats |
| 14 | `/me` actions counted as lines | pisg-class convention | **in-model** | Counted as messages **and** tallied separately |
| 15 | Machine-readable output for scripting | 3 (library output) | **in-model** | `output = json` |
| 16 | Emoji stats | 2 | **in-model but scoped out here** | Belongs to `whatsapp-chat-analyzer` (already ships it); IRC logs use text emoticons — see #17 |
| 17 | Emoticon stats (`:)`, `:D`) | 1 | **out of scope (deliberate)** | Not built: low signal vs. the word ranking, and would duplicate the top-words section |
| 18 | Charts / pie charts / bar graphs / heatmaps / word clouds | 1, 2 | **out-of-model** | This block returns text/JSON; no chart rendering surface. ASCII bar histograms are the in-model substitute |
| 19 | NLP topic extraction beyond stopword filtering (NLTK-class) | 2 | **out-of-model** | Needs an ML/NLP model; gizza blocks are pure Rust. `rake-keywords` is the nearest existing block |
| 20 | Multi-channel / multi-file corpus analysis ("most active channels") | 3 | **out-of-model** | Single-text input surface; one log at a time |
| 21 | Persistent log database + cross-session queries | chatdb | **out-of-model** | No storage in a pure block |
| 22 | Telegram JSON / WhatsApp TXT export parsing | 2 | **out of scope (deliberate)** | WhatsApp is covered by the existing `whatsapp-chat-analyzer` block; this tool targets IRC/generic line logs |
| 23 | User-flow / social-graph analysis (who replies to whom) | 3 | **out-of-model for v1** | Requires reply threading heuristics that IRC line logs don't reliably carry |

## Dup check (why this is not `whatsapp-chat-analyzer`)

`blocks/whatsapp-chat-analyzer/core/src/lib.rs` recognises exactly two dialects — the iOS
bracketed export (`[2024-01-05, 21:07:33] Alice: hey`) and the Android dash export
(`05/01/2024, 21:07 - Alice: hey`). An IRC line (`21:07 <alice> hey`), an irssi/WeeChat line,
or a bare `alice: hey` line parses to **zero messages** there. This block is the IRC/generic
counterpart: angle-bracket nicks with mode prefixes, `/me` actions, join/part/quit/nick/kick
events, tab-separated WeeChat columns, and nick exclusion for bots — none of which the
WhatsApp block models. Emoji ranking is deliberately left to the WhatsApp block so the two
don't converge.

## UX patterns adopted

- **Preset chips** (`[[example]]`): three one-click samples — an irssi-style dated log, a
  bare `nick: message` log, and a "keep every word" no-filter run. Competitors ship sample
  logs / demo reports; chips are the declarative equivalent here.
- **`tag-list` control** for `exclude_nicks` so bot names are added as removable pills rather
  than hand-typed CSV.
- **`[input.labels]`** friendly labels on the `output` select (`Readable report` / `JSON`).
- **Multiline textarea** for the log itself (pasted logs are newline-heavy; a plain input
  strips newlines).
- **Non-empty-rows-only histograms** — a 24-row hour table of mostly zeros reads as noise;
  competitors' bar charts have the same effect visually.

## Stated limits (also on the page)

- One log at a time; no multi-channel corpus rollups.
- Times are used exactly as written in the log — no timezone conversion.
- Ambiguous numeric dates (`05/01/2024`) are auto-resolved: a first field > 12 means
  day-first, a second field > 12 means month-first, otherwise day-first. ISO dates are
  unambiguous and preferred.
- Weekday stats need dated lines; a time-only log still gets full hour stats.
- Stopword filtering is English-only.
- Input is capped at 5,000,000 characters.

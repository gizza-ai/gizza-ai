# telegram-export-reader — competitor analysis (2026-07-17)

Snapshot taken during the initial build of `telegram-export-reader` (pure tool: paste a
Telegram Desktop `result.json` export → clean transcript + per-sender message/word stats).
All competitor notes are **paraphrased** — no copy, branding, or trademarks reproduced.

## Competitors scanned (top 5)

| tool | url | what it does | input | output | free/paid |
| ---- | --- | ------------ | ----- | ------ | --------- |
| Petraller Telechat | petraller.com/telechat | Closest direct competitor — client-side analysis of a Telegram export | `result.json` | On-page stats (total + per-sender counts, sticker-type breakdown) | Free |
| Telegramalyzer | telegramalyzer.github.io | Browser-local chat statistics ("who writes more", top emoji, time comparison) | Chat log via a 3rd-party extension | Charts + stat breakdowns | Free |
| CoupleFlow Telegram Chat Analyzer | coupleflow.app/telegram-chat-analyzer | One-off relationship/communication analysis | JSON export | On-page summary + AI insights | Free tool, paid app upsell |
| BitRecover Chat Converter | bitrecover.com (desktop .exe) | Bulk-converts exports to readable file formats | Bulk JSON | 27+ file formats | Freemium |
| Telegram Web Chat Exporter (Chrome ext.) | Chrome Web Store | Saves an open web.telegram.org chat to a file | Live web DOM | HTML / TXT | Free |

Also-relevant open scripts/libraries (not hosted tools, confirm demand): keizerzilla/telegram-chat-parser
(JSON→CSV), LoadingByte/telegram-text-extractor, Infinidrix/telegram-data-analyzer,
pdonadeo/telegram-json-converter (JSON→HTML), cyb3rm4gus/telegram-filtered-chat-parser (per-user filter).

## Table-stakes → decision

| capability | fit | shipped here |
| ---------- | --- | ------------ |
| Accept Telegram Desktop `result.json` (single-chat AND full-export shapes) | in-model | ✅ core parses `messages`, `chats.list`, or a bare array |
| Fully client-side / no upload | in-model | ✅ pure wasm, stated on the page |
| Total message count | in-model | ✅ stats header |
| Per-sender message count | in-model | ✅ per-sender table w/ share % |
| Per-sender **word** count | in-model | ✅ per-sender table ("N words") |
| Word-frequency / most-used words | in-model (differentiator) | ✅ "Top words" list in stats |
| Emoji usage stats | in-model | ✅ "Top emoji" list in stats |
| Sticker / message-type breakdown | in-model | ✅ media/service message counts + transcript placeholders (`[photo]`, `[sticker 🎉]`, …) |
| Filter by sender | in-model | ✅ `sender_filter` param |
| Clean readable transcript | in-model | ✅ `output = transcript` |
| Output-mode toggle (transcript / stats / both) | in-model | ✅ `output` enum |
| Include/exclude Telegram service messages | in-model | ✅ `include_service_messages` bool |
| Cap huge exports | in-model | ✅ `max_messages` (0 = all) |
| Built-in sample export to try | in-model (easy win, none of the competitors clearly ship one) | ✅ `[[example]]` preset chips |

## Considered, not built

- **Date-range filter** — in-model, but the requested param set is transcript/stats/service/sender/cap;
  `max_messages` + `sender_filter` cover the common trimming need. Left out to keep the schema focused;
  could be a follow-up param.
- **Rendered activity-by-hour/day charts** — computing the buckets is in-model, but a rendered chart needs
  a charting lib; the page format is plain text. Out of scope.
- **Media/image extraction + base64 embedding** — [out-of-model] needs the actual media files, which a
  `result.json` paste does not contain.
- **AI-generated insights/summaries** — [out-of-model] needs a server/ML model.
- **Shareable result links, DOCX/PDF export, live-web-DOM scraping** — [out-of-model] need a server or
  browser extension.

## UX patterns matched

- Preset "Try a sample export" chips (a differentiator — competitors rarely ship one).
- Loud privacy note at the input (runs locally, nothing uploaded).
- Output-mode toggle and a sender filter, mirroring the stats/transcript split competitors offer.
- A "how to export" note in the page copy (Settings → Advanced → Export chat history → Machine-readable JSON).

## SEO / copy angles (paraphrased)

Read a Telegram `result.json` export as a clean transcript; per-person message & word counts; most-used
words and emoji; private/offline/no-account; "how to export Telegram chat history to JSON".

# Competitor analysis — whatsapp-chat-analyzer (2026-07-17)

Function: parse an exported WhatsApp chat (`_chat.txt` / Android `.txt`) and report per-person
message counts, busiest hours and days, and word + emoji frequency. Browser-local, no upload.

All findings are **paraphrased** from public marketing/landing pages — no competitor copy,
branding, or trademarks are reproduced. Several competitors are client-side SPAs whose landing
pages render via JS, so feature lists were taken from their visible marketing text and general
category conventions.

## Competitors scanned

1. **Chatilyzer** (chatilyzer.com) — upload an exported `.txt`; reports the top sender ("who
   sends the most"), the most-used emoji, and daily message volume. Stores derived stats
   (common emojis/words/active users) server-side for shareable visualizations; states data is
   deleted after 72 hours.
2. **WhatsAnalyze** (whatsanalyze.com) — accepts the WhatsApp `.zip` or `.txt`; positions itself
   as processing in-memory and deleting data immediately after analysis. Timeline + per-person
   breakdowns.
3. **ChatAnalyse** (chatanalyse.com) — explicitly "chat data never leaves your browser," fully
   local; who-sends-most, message statistics, free.
4. **Chatistics** (chatistics.vercel.app) — open-source, positioned on privacy; standard
   analytics/insights over an export.
5. **DoubleText** (doubletext.me/whatsapp) — WhatsApp chat analyzer with fun-fact style insights.

Several open-source Python scripts (ChatStatsForWhatsApp, WhatsAppChatAnalyzer) confirm the
category's table-stakes metrics: message counts, media usage, temporal patterns (hours/days),
most-used words and emojis.

## Table-stakes metrics (category consensus)

| metric | in our tool? | notes |
| --- | --- | --- |
| Messages per person + share | ✅ | ranked desc, with % of total |
| Total messages / date range / participant count | ✅ | overview block |
| Busiest hour-of-day | ✅ | ranked 0–23 histogram |
| Busiest day-of-week | ✅ | Mon–Sun ranked |
| Word frequency (top N) | ✅ | with stop-word + min-length filters |
| Emoji frequency (top N) | ✅ | ZWJ/flag-aware clustering |
| Media message count | ✅ | detects `<Media omitted>` / `image omitted` / `<attached:` |
| Link count | ✅ | messages containing a URL |
| Handles iOS `[..]` and Android `..-` line formats | ✅ | auto-detects both |
| Ambiguous DD/MM vs MM/DD dates | ✅ | `date_format` = auto/dmy/mdy/ymd (auto-detects) |

## UX controls competitors ship

- **File upload of the export** — out of model for the page's field-driven form; we accept the
  pasted chat text in a `multiline` textarea (the CLI/chat surfaces take the same string). Users
  open `_chat.txt` and paste, or pipe the file on the CLI.
- **Preset "try it" example** — we ship an `[[example]]` chip with a tiny sample chat.
- **Top-N control for words/emoji** — we expose `top` (default 10).

## In-model decisions (built)

- `date_format` (enum auto/dmy/mdy/ymd) — resolves the DD/MM vs MM/DD ambiguity that silently
  corrupts "busiest day" in naive parsers; auto-detects from the data when possible.
- `top`, `min_word_length`, `ignore_stopwords` — shape the word/emoji leaderboards.
- Both export dialects (iOS bracketed with seconds/AM-PM, Android dash) parsed without regex.
- Multi-line messages folded into the preceding message; system/notification lines excluded from
  per-person totals.

## Out-of-model (considered, not built)

- **`.zip` upload / media file extraction** — needs a file picker + unzip of attachments;
  the page form is field-based. The text report already counts media placeholders.
- **Server-stored shareable dashboards / images** (Chatilyzer) — needs a backend + storage;
  conflicts with the browser-local, no-account model. We emit a copy-paste text report instead.
- **Interactive charts / heatmaps** — the generic text page renders a plain report; charting
  would need a bespoke renderer. Ranked histograms in text cover the same information.
- **Sentiment / AI summaries** — needs an ML model; gizza pure tools are deterministic, no model.
- **Response-time / who-texts-first analysis** — feasible but adds heavy schema; deferred as a
  later capability, not a launch table-stake.

# bot-traffic-filter — competitor analysis (2026-07-26)

**Tool function:** paste an access log or event list, classify each hit as bot/crawler vs
human by its user-agent string, strip the bot hits, and report the human-versus-bot split.

**Scan basis:** one WebSearch for the tool's function; skimmed the top real competitors and
reference material below (paraphrased — no copy, branding, or trademarks reproduced):

1. **PostHog — bot/crawler detection docs.** Ships a maintained deny-list of user-agent
   substrings ("bot", "crawl", "spider", "slurp", plus specific agents) and classifies traffic
   into buckets (regular / AI agent / bot / automation). Table-stake: a curated token list,
   category buckets, and a per-request bot/human flag.
2. **51Degrees — "filter bots from analytics with user-agent data."** Emphasises matching the
   UA against known crawler patterns and, as a secondary signal, browser-release age / library
   UAs (curl, python-requests, Go-http-client). Table-stake: HTTP-library UAs count as bots even
   without a "bot" token.
3. **AI Crawler Check / Botify — log-file analysis for AI bots.** Break traffic down *by bot
   name* (GPTBot, ClaudeBot, PerplexityBot, Bytespider…) and by request volume; server logs are
   the reliable source because GA4 already hides most bots. Table-stake: name the specific bot,
   group by category, count per bot.
4. **HUMAN Security — "ultimate list of crawlers/known bots."** A large public directory of bot
   user-agent tokens grouped by purpose (search, SEO, monitoring, social preview, AI). Confirms
   the category taxonomy below.

## Table-stakes → decision (in-model vs out-of-model)

| Capability | Decision | Where |
|---|---|---|
| Classify each line bot vs human by user-agent | in-model | `output=table/json/csv` |
| Curated known-bot token list (search/AI/SEO/monitoring/social/library/headless) | in-model | core `classify()` |
| Name the specific bot (Googlebot, GPTBot, curl…) | in-model | `bot` column |
| Category breakdown (search-engine, ai-crawler, …) | in-model | `report` output |
| Human-vs-bot split: counts + percentages | in-model | `report` output |
| **Strip** bots → keep only human lines | in-model | `output=humans` |
| Keep only bot lines (inverse) | in-model | `output=bots` |
| Parse UA out of a Combined-Log-Format access line | in-model | `format=auto/combined` |
| Accept a bare user-agent list (one per line) | in-model | `format=auto/plain` |
| Treat empty / "-" user-agent as a bot | in-model | `empty_is_bot` (default true) |
| Top-bots ranking in the summary | in-model | `report` output |
| IP-range / reverse-DNS verification of declared crawlers | out-of-model | needs live DNS/network; listed, not built |
| Behavioural / rate / fingerprint / JS-challenge detection | out-of-model | needs a server + session state |
| Spoofed-UA detection (UA says Googlebot, IP disagrees) | out-of-model | needs IP intelligence; noted on page |
| Live GA/CDN integration, dashboards, accounts | out-of-model | gizza is browser-local, no backend |

## UX controls competitors ship (and our answer)

- Format selector (access log vs UA list) → `<select>` from `format` enum + friendly labels.
- Output mode (report / strip / keep-bots / table / data) → `<select>` from `output` enum.
- Preset examples (an access log, a UA list, "strip bots") → `[[example]]` preset chips.
- Empty-UA-as-bot toggle → boolean checkbox (`empty_is_bot`, default on).

## Worked example (used on the page)

Input (3 combined-log lines):
```
1.1.1.1 - - [26/Jul/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
2.2.2.2 - - [26/Jul/2026:10:00:01 +0000] "GET /robots.txt HTTP/1.1" 200 55 "-" "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"
3.3.3.3 - - [26/Jul/2026:10:00:02 +0000] "GET /api HTTP/1.1" 200 3 "-" "python-requests/2.31.0"
```
`output=report` → 3 hits · humans 1 (33.3%) · bots 2 (66.7%); categories: search-engine 1,
library 1; top bots: Googlebot 1, python-requests 1.
`output=humans` → the first line only (bots stripped).

Paraphrase-only; the token list is drawn from public user-agent conventions, not any single
vendor's proprietary list.

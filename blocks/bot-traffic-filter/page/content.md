## What this tool does

Paste raw access-log lines or a plain list of user-agent strings and the **Bot Traffic
Filter** classifies every hit as **human** or **bot/crawler** by its user-agent, strips the
bot hits, and reports the human-versus-bot split. It recognises search-engine crawlers
(Googlebot, Bingbot, Yahoo! Slurp), AI crawlers (GPTBot, ClaudeBot, PerplexityBot, CCBot,
Bytespider), SEO tools (AhrefsBot, SemrushBot, MJ12bot), uptime/monitoring probes, social
link-preview fetchers, generic HTTP libraries (curl, Wget, python-requests, Go-http-client,
okhttp), and headless browsers — plus the standard `bot` / `crawl` / `spider` / `slurp` token
heuristic and an optional "missing user-agent is a bot" rule.

Everything runs locally in your browser — nothing is uploaded, and there are no accounts or
API keys.

## Worked example

Input (four Combined Log Format lines), with **Output as = Report**:

```
1.1.1.1 - - [26/Jul/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/125.0 Safari/537.36"
66.249.66.1 - - [26/Jul/2026:10:00:01 +0000] "GET /robots.txt HTTP/1.1" 200 55 "-" "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"
20.15.1.9 - - [26/Jul/2026:10:00:02 +0000] "GET /blog HTTP/1.1" 200 800 "-" "Mozilla/5.0 (compatible; GPTBot/1.2; +https://openai.com/gptbot)"
45.9.8.7 - - [26/Jul/2026:10:00:03 +0000] "GET /api HTTP/1.1" 200 3 "-" "python-requests/2.31.0"
```

Output:

```
Bot traffic report
==================
Total hits:  4
Human:       1 (25.0%)
Bot:         3 (75.0%)

Bots by category:
  Search engines           1
  AI crawlers              1
  HTTP libraries / scripts 1

Top bots:
  GPTBot                   1
  Googlebot                1
  python-requests          1
```

Switch **Output as** to **Human lines only (strip bots)** to get back just the first line, or
to **Table / JSON / CSV** for a per-hit breakdown you can paste elsewhere.

## How detection works

The user-agent is matched **case-insensitively** against a curated list of known crawler and
agent tokens, then against the broad `bot` / `crawl` / `spider` / `slurp` token heuristic. In
**Auto** mode the user-agent is taken from the last quoted field of an access-log line, or —
when a line has no quoted fields — the whole line is treated as a user-agent, so a plain list
of user-agents works too. **Combined** forces Apache/nginx Combined Log Format; **User-agent
list** treats every line as a bare user-agent.

## Limits & edge cases

- Detection is by **declared user-agent only** — it does not verify crawlers by IP range or
  reverse DNS, so a request that spoofs `Googlebot` is classified as Googlebot. For
  authoritative verification, confirm the source IP against the operator's published ranges.
- No behavioural, rate, fingerprint, or JavaScript-challenge signals — those need a live
  server and session state.
- A missing or `-` user-agent counts as a bot when **Treat a missing user-agent as a bot** is
  on (the default); turn it off to count blank-UA hits as human.
- The broad `bot` token is a substring match, so an unusual product name containing "bot" can
  be flagged — rare in practice but worth knowing.
- The table/JSON/CSV/line outputs are capped by **Max rows** (default 500, up to 10000); the
  Report summary always counts every line regardless of the cap.
- Blank lines are skipped and not counted.

## FAQ

<details>
<summary>What log formats can I paste?</summary>

Apache/nginx **Combined Log Format** (the user-agent is the last quoted field) and plain
**user-agent lists**, one entry per line. In Auto mode the tool detects which one each line is;
you can also force **Access log (Combined)** or **User-agent list** with the format selector.
The **Common** log format has no user-agent field, so paste the combined variant if you want
bot detection.

</details>

<details>
<summary>How does it tell bots from humans?</summary>

By matching the user-agent string, case-insensitively, against a curated list of known
crawlers and agents — search engines, AI crawlers, SEO tools, monitoring probes, social
link-preview fetchers, HTTP libraries and headless browsers — plus the standard
`bot`/`crawl`/`spider`/`slurp` token heuristic. It does **not** use IP ranges or behaviour, so
a user-agent that lies about who it is will be taken at its word.

</details>

<details>
<summary>Does it detect AI crawlers like GPTBot and ClaudeBot?</summary>

Yes. GPTBot, OAI-SearchBot, ChatGPT-User, ClaudeBot, Claude-Web, anthropic-ai, PerplexityBot,
Google-Extended, CCBot, Bytespider, Amazonbot, Meta-ExternalAgent and several others are
tagged in the **AI crawlers** category, so you can see how much of your traffic is AI training
and answer-engine crawling.

</details>

<details>
<summary>How do I remove bots and keep only real visitors?</summary>

Set **Output as** to **Human lines only (strip bots)** — the tool returns the original log
lines that were classified as human, with every bot line removed, ready to copy. **Bot lines
only** gives you the inverse.

</details>

<details>
<summary>Is my log data uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser tab. Your log text
never leaves your machine — there is no server, account, or upload.

</details>

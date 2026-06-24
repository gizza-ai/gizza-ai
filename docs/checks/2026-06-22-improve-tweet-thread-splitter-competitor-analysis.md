# tweet-thread-splitter — competitor analysis (2026-06-22)

Tool: split long text into numbered, character-limit-safe tweet chunks that
never break mid-word. Surfaces verified: chat (wafer fixtures), CLI, page
(Playwright) — all green.

## Competitors surveyed

1. **SimpliConvert — Twitter Thread Formatter** (simpliconvert.com/twitter_thread_formatter)
2. **Tools Fast — Twitter Thread Splitter** (toolsfast.app/writing/thread-splitter)
3. **Tool IQ Hub — Twitter Thread Splitter** (tooliqhub.com/twitter-thread-splitter)
4. **t.ly — Twitter Thread Maker** (t.ly/tools/twitter-thread-maker)
5. **CyberTrickz — Tweet Splitter** (cybertrickz.info/tweet-splitter-tool)
6. **Tweet Thread Splitter — Chrome extension** (chromewebstore.google.com)

## Feature matrix (competitor consensus vs. gizza)

| Capability | Competitors | gizza (after this pass) |
| --- | --- | --- |
| Custom character limit | Yes (default 280) | Yes — `limit`, default 280, 10–25000 |
| Never break a word mid-word | Yes (core promise) | Yes (guaranteed; over-long words like URLs hard-split) |
| Reserve numbering space in the count | Yes | Yes — counter counts toward the per-tweet limit |
| (i/N) numbering | Yes | Yes — `numbering=parens` (default) |
| Alternate counter styles (i/N, numbered-list) | Some (start/end/skip; "1." style) | **Added** — `numbering = parens \| slash \| dotted \| none` |
| Sentence-boundary splitting (don't end mid-thought) | Yes (common differentiator) | **Added** — `prefer_sentences` (default on), falls back to word-packing |
| Emoji / UTF-16-aware counting | Rarely | Yes — `count = chars \| utf16` (we lead here) |
| Live "tweets it will generate" preview | Yes (their JS UI) | Page recomputes output on input; generic page driver has no per-tweet count badge |
| One-click copy per tweet / whole thread | Yes | Out of model — the generic tool page renders text output with the shared copy affordance; no per-tweet copy buttons |
| Manual split marker (`[…]`) | One Chrome ext only | Not built — niche; would need an in-text sentinel convention |
| Custom prefix/suffix per tweet | A few | Not built this pass — low marginal value vs. numbering styles already added |
| Privacy / local / no sign-up | Yes | Yes — runs entirely client-side (page) / in the WASM sandbox (chat) |

## Gaps closed in this pass

- **Numbering style choice** (`numbering` enum: `parens` / `slash` / `dotted` /
  `none`) — matches competitors that offer "(1/n)" vs plain "1/n" vs numbered-list
  "1." styles, replacing the original boolean toggle.
- **Sentence-aware breaks** (`prefer_sentences`, default on) — starts a new tweet
  after `. ! ?` where it fits, so tweets rarely end mid-thought; a sentence longer
  than the limit still word-packs. This was the single most-cited competitor
  differentiator.

## Gaps intentionally NOT closed (out of model / low value)

- **Per-tweet copy buttons / live tweet-count badge** — UI affordances owned by the
  shared generic tool page, not expressible in the tool descriptor. Out of model.
- **Manual `[…]` split marker** — niche (one Chrome extension); an in-text sentinel
  convention adds surface area for little gain; deferred.
- **Custom prefix/suffix text** — marginal once numbering styles exist; deferred to
  keep the schema tight.

## Notes

- We do NOT copy any competitor copy, branding, or trademarks — all page copy and
  schema descriptions are original.
- We lead competitors on UTF-16-aware counting (most only count raw characters),
  which matters for emoji-heavy threads on X.

Sources: SimpliConvert, Tools Fast, Tool IQ Hub, t.ly, CyberTrickz, and the
"Tweet Thread Splitter" Chrome Web Store listing (surveyed 2026-06-22).

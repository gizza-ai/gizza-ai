# hashtag-extractor — competitor analysis (2026-09-04)

Scan run **before** implementation so the first shipped version already covers the table stakes.
All notes below are **paraphrased observations** of publicly reachable tool pages. No competitor
copy, branding, logos, or trademarks were reproduced; every string in our block/page is original.

Two 403-blocked candidates (codeshack.io, freetexttools.org, tooliqhub.com) were replaced with the
next reachable real tools per the skill's "replace, don't run with 4" rule.

## Profiles

### 1. GuinRank — Text to Hashtags
- **URL:** https://www.guinrank.com/text/text-to-hashtags
- **Features:** relevance-scored keyword→hashtag generation; per-platform tag-count targets with
  live counters (their page cites Instagram 30 / X 3 / TikTok 20 / LinkedIn 5); manual custom-tag
  chips; Unicode-aware tokenisation across scripts; sort by score / alphabetical / tag length;
  copy + TXT download.
- **Params/options:** text area (paste or file), platform selector with +/− max count, stop-word
  filter toggle, lowercase toggle, sort selector, language/RTL selector, clear/reset.
- **Output:** tag list with per-tag relevance score, total tag count, total character length,
  input-word and filtered-word counts.
- **Limits / pricing:** free tier metered (a stated daily quota); account required for saving and
  the "pro" surface.
- **UX:** live recompute, progress bars against the platform cap, chip UI with × removal.
- **SEO angles:** "turn a caption/sentence into hashtags", per-platform hashtag counts, multilingual.

### 2. Toolszu — Text to Hashtags
- **URL:** https://toolszu.com/tools/text-to-hashtags/
- **Features:** stop-word removal, punctuation/spacing normalisation, duplicate suppression,
  fully client-side, output aimed at Instagram / X / LinkedIn / TikTok / Facebook.
- **Params/options:** essentially none — a text area and a convert button. Fixed behaviour:
  lowercase, strip punctuation, drop stop words, drop words under 3 characters.
- **Output:** flat hashtag list + a generated-count readout.
- **Limits / pricing:** free, unlimited, no signup; no stated text-size cap.
- **UX:** three-step paste → convert → copy; reset button; bulk copy.
- **SEO angles:** platform compatibility, "does it handle a plain keyword list", why common words
  are dropped, duplicate handling, is it free.

### 3. TextSorter — Hashtag Extractor
- **URL:** https://textsorter.com/hashtag-extractor/
- **Features:** pulls hashtags that are **already written** in the text (regex on `#`), duplicate
  removal, optional stripping of the `#` sign, live count.
- **Params/options:** output separator (space / comma / newline; space default), remove duplicates
  (on by default), remove `#` (off by default), wrap text.
- **Output:** the separator-joined tag list; copy + download.
- **Limits / pricing:** free, client-side, no stated volume cap.
- **UX:** paste-and-go, live preview of the extracted count.
- **SEO angles:** competitor-post analysis, campaign tag inventory, privacy/local processing.

### 4. OnAirCode — Hashtag Extractor
- **URL:** https://onaircode.com/hashtag-extractor/
- **Features:** extracts existing `#tags`, deduplicates, lowercases, sorts alphabetically, exports.
- **Params/options:** format (one-per-line default, or comma separated), remove duplicates toggle,
  lowercase toggle, sort (none default, or A–Z).
- **Output:** line- or comma-delimited tag list; paste / copy / download buttons.
- **Limits / pricing:** free, no tier.
- **UX:** single-page form, collapsible FAQ.
- **SEO angles:** roles (content manager, marketer, researcher), analytics prep, consistency.

### 5. WebTextTools — Extract Hashtags
- **URL:** https://webtexttools.com/htmltools/extract-hashtags/
- **Features:** extract / remove / alphabetically-sort modes over text or HTML, duplicate removal,
  line-break preservation, local processing, TXT/CSV/HTML download, tool history.
- **Params/options:** mode (extract default), duplicates (on), preserve line breaks (on).
- **Output:** one tag per line; multiple download formats.
- **Limits / pricing:** free, no registration.
- **SEO angles:** content audits, campaign analysis, trend tracking, privacy.

## Cross-competitor read

The market splits into two families that no reachable free tool combines well:

- **Generators** (GuinRank, Toolszu) — score keywords out of prose and emit new hashtags.
- **Extractors** (TextSorter, OnAirCode, WebTextTools) — regex out the `#tags` already present.

Table stakes across both: stop-word filtering, a minimum word length, duplicate removal, casing
control, a choice of output separator, a tag/character counter, and platform-count awareness.

Platform guidance is also visibly **stale** on the generator side: GuinRank's page still advertises
30 Instagram tags, while 2026 guidance across social-marketing sources has converged on a handful
of highly relevant tags per post (Instagram/TikTok ~5, X ~2, LinkedIn ~5, Facebook ~3).

## Gap list → decisions

| # | Gap (≥1 competitor has it) | Dimension | Fit | Decision |
| - | -------------------------- | --------- | --- | -------- |
| 1 | Score keywords by relevance, not just frequency | capabilities | in-model | **Built** — score = occurrences × an earlier-in-text bonus × phrase length; exposed per tag. |
| 2 | Keep hashtags already written in the text | capabilities | in-model | **Built** — `include_existing` (default on); authored tags are emitted verbatim and listed first. Combines both competitor families in one tool. |
| 3 | Per-platform tag counts | capabilities | in-model | **Built** — `platform` enum (`none`/`instagram`/`tiktok`/`x`/`linkedin`/`facebook`) caps the output at the 2026 recommended count; the page states the numbers are current guidance, not hard platform maxima. |
| 4 | Casing control (lowercase toggle) | capabilities | in-model | **Built and widened** — `style` enum: `lowercase`, `camel`, `pascal`, `preserve` (multi-word tags need CamelCase for screen readers, which no scanned tool offered). |
| 5 | Output separator choice | capabilities | in-model | **Built** — `separator` enum: `space`, `comma`, `newline`. |
| 6 | Minimum word length | capabilities | in-model | **Built** — `min_word_length` (default 3, matching the family norm). |
| 7 | Duplicate removal | capabilities | in-model | **Built, always on** — case-insensitive dedupe; a "keep duplicates" switch has no real use for hashtags, so it stays out of the schema. |
| 8 | Multi-word keyphrase tags | capabilities | in-model | **Built** — `phrase_words` (1–4). Longer phrases that fully contain a shorter one with the same frequency suppress it, so `#content #marketing #contentmarketing` collapses to `#contentmarketing`. |
| 9 | Tag count + character count readout | copy/UX | in-model | **Built** — the page result footer reports tags, characters, and candidates found. |
| 10 | Alphabetical / length sorting | capabilities | in-model | **Considered, rejected** — relevance order is the point of the tool, and the output is a short line users reorder by hand. Adding a third ordering knob is schema bloat for no reach benefit. |
| 11 | Strip the `#` from results | capabilities | in-model | **Considered, rejected** — this tool's job is producing hashtags; the plain keyword list is already served by `blocks/rake-keywords`. |
| 12 | HTML input, remove-tags mode, CSV/HTML download | capabilities | in-model but off-scope | **Considered, rejected** — that is a text-cleanup tool, not a hashtag generator; the page's copy/download covers the real need. |
| 13 | Custom manually-added tag chips | UX | in-model | **Considered, rejected** — a user can paste their own `#tags` into the input and `include_existing` keeps them, which reaches the same outcome without a second field. |
| 14 | Trending/volume data per hashtag, competitor tag mining | capabilities | **out-of-model** | Not built — needs a server, a social API, and credentials. gizza tools are browser-local and account-free. |
| 15 | File upload of a document to tag | capabilities | out-of-model here | Not built — this is a pure text tool; document extraction is a separate block family. |
| 16 | Saved history / accounts / daily quotas | UX | **out-of-model** | Not built (and not wanted) — no accounts, no metering, nothing leaves the browser. |
| 17 | Non-Latin script support | capabilities | in-model | **Built by construction** — tokenisation is Unicode `is_alphanumeric`, so Cyrillic/Greek/CJK words tokenise; the built-in stop-word list is English only, which the page states plainly. |

## Positioning

Ours is the only one of the six that does generation **and** extraction in a single pass, exposes
the relevance score, and runs with no account, no quota, and no upload. Its honest weak spot versus
GuinRank is the English-only stop-word list, which is stated on the page rather than hidden.

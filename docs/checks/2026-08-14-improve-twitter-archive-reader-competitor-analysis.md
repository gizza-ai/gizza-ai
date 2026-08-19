# Competitor analysis — twitter-archive-reader (2026-08-14)

Tool: `twitter-archive-reader` — parses a Twitter/X data export (`tweets.js`) into a readable
Markdown/text/HTML/CSV transcript plus posting statistics and top tweets.

Scan performed 2026-08-14, before implementation. Sources skimmed (top results for
"Twitter/X archive parser / tweets.js viewer / archive to markdown"):

1. **timhutton/twitter-archive-parser** (open-source Python CLI) — converts an extracted archive to
   Markdown and HTML, replaces `t.co` links with the expanded URLs found in the archive, embeds
   media from the archive folder, and also handles DMs and follower lists. Optionally queries
   Twitter for missing handles and downloads original-size images.
2. **Xarchive** (browser archive viewer) — upload `tweets.js`, then search and filter old tweets
   (date, engagement, media type), card-based list, one-click "open this post on X", explicit
   "stays in your browser" privacy claim, "thousands of tweets in seconds" performance claim.
3. **doggy8088/x-archive-parser** (Node CLI) — strips the `window.YTD.tweet.part0 = …` JavaScript
   wrapper automatically, exports to Excel with a summary sheet (total tweets, retweets, replies,
   likes/retweets received, date range, top languages), a hashtag sheet (counts, percentages, top
   100) and a per-tweet sheet (engagement, language, source app, mentions, URLs).
4. **xposterai Twitter/X Archive Viewer** (browser) — tab per data category (posts, replies,
   reposts, media, likes, DMs, followers), sort by likes or retweets, engagement-rate metrics,
   in-browser-only processing, warns that multi-GB zips can exhaust browser memory.

No competitor copy, branding or trademark is reproduced anywhere in this tool. "Twitter" and "X"
appear only as factual descriptions of the file format being read.

## Table stakes

| Capability / UX pattern | Decision |
| --- | --- |
| Accept `tweets.js` verbatim, including the `window.YTD.tweets.part0 = [ … ]` JavaScript wrapper. | In model. The wrapper (any `part` index, `tweet` or `tweets` key) is stripped; a bare JSON array and bare tweet objects without the `{"tweet": …}` envelope are also accepted. |
| Expand `t.co` short links back to the original URLs stored in the archive. | In model. `expand_urls` (on by default) rewrites each `entities.urls[]` `t.co` link to its `expanded_url` and drops the redundant `t.co` media link from the text. |
| Markdown output; HTML output. | In model. `format` = `markdown` (default), `text`, `html`, `csv`. CSV is our machine-readable answer in place of Excel. |
| Keyword search across tweet text. | In model. `search` is a case-insensitive substring filter applied after t.co expansion. |
| Date-range filtering. | In model. `since` / `until` inclusive `YYYY-MM-DD` UTC bounds. |
| Separate/omit replies and retweets. | In model. `include_replies` and `include_retweets` checkboxes; every tweet is classified original / reply / retweet in the transcript and counted in the summary. |
| Sort by newest, oldest, likes, or retweets. | In model. `sort` enum, default `newest` (matching the viewers' default reverse-chronological feed). |
| Link each tweet back to the live post. | In model. Every tweet renders its canonical `https://twitter.com/i/web/status/<id>` permalink (works without knowing the account handle, which `tweets.js` does not contain). |
| Summary metrics: totals, originals/replies/retweets, likes and retweets received, averages, date range, languages. | In model. Rendered as the summary block of the `stats` output. |
| Hashtag analysis with counts and share percentages. | In model. Top hashtags table with counts and percentage of matched tweets; mentions and link domains get the same treatment. |
| Source/client app breakdown. | In model. `source` is unwrapped from its HTML anchor and tallied. |
| Top tweets by engagement. | In model. `top_count` (default 5) most-liked tweets, with likes, retweets, date and permalink. |
| Per-year activity breakdown. | In model. Tweets-per-year rows in the summary. |
| Local-only processing, nothing uploaded. | In model. Pure Rust compiled to WebAssembly; the page runs it in the browser and the CLI runs it locally. |
| Cap for previewing very large archives. | In model. `max_tweets` (0 = all) is applied after filtering and sorting, and the truncation is reported. |
| Embed media (images/video) from the archive's `media/` folder. | Out of model — the tool reads the `tweets.js` text only, not the surrounding files. Media is listed as `[photo: url]` / `[video: url]` placeholders from the archive metadata. |
| Upload the whole `twitter-archive.zip`. | Out of model for this block. Extract the zip first (the toolkit's `archive-extractor` handles zips) and paste `tweets.js`. |
| DMs, likes, followers/following, ad interests, Grok chats, deleted posts. | Out of model — each lives in a different export file (`direct-messages.js`, `like.js`, `follower.js`, …). This tool is scoped to `tweets.js`. |
| Excel `.xlsx` export with multiple worksheets. | Out of model (binary spreadsheet writing). `format = "csv"` covers the machine-readable path. |
| Querying the Twitter/X API for missing handles or original-size images. | Out of model — the block is offline and deterministic; no network calls. |
| AI persona training / tweet rewriting / scheduling. | Out of model — needs a model and an account. |
| Interactive card feed with infinite scroll and per-card actions. | Out of model for the generic tool-page shape. The equivalent UX is delivered through preset example chips, `?param=` deep links, sort/search/date controls and the copyable rendered output. |

## Built checks

- Wrapper stripping (`window.YTD.tweets.part0 = …`), bare array, and bare (un-enveloped) tweet objects.
- Classification of original / reply / retweet, including the `RT @user:` retweet form.
- `t.co` expansion on and off, and HTML entity decoding (`&amp;`, `&lt;`, `&gt;`).
- All four formats (markdown, text, html, csv) and all three outputs (transcript, stats, both).
- All four sort orders, the search filter, the date bounds, both checkboxes off, and the
  `max_tweets` cap at its exact boundary.
- Stats: totals, engagement sums/averages, per-year rows, top hashtags/mentions/domains/sources/
  languages, and the top-N most-liked tweets.
- Error paths: empty input, non-JSON input, JSON that is not an array, an array with no tweet
  objects, reversed date bounds, and an unknown format/sort value.

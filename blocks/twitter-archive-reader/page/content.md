## What this tool does

Paste the `tweets.js` file from a Twitter/X data export and get something you can actually read: every post with its UTC timestamp, likes, retweets, language and posting app, classified as original, reply or retweet, plus a summary of how you posted over the years. The archive is parsed in your browser — nothing is uploaded, and no account or API key is involved.

The export is not JSON. `data/tweets.js` is a JavaScript assignment, `window.YTD.tweets.part0 = [ … ]`, wrapping an array of `{"tweet": {…}}` envelopes with `t.co` short links, HTML-escaped text and engagement counts stored as strings. This tool strips the wrapper, expands each `t.co` link back to the URL the archive already stores for it, decodes the entities, and renders the result as Markdown, plain text, HTML or CSV.

## Worked example

Input — a two-tweet `tweets.js` (an original with a shortened link, and a reply):

```js
window.YTD.tweets.part0 = [
  {"tweet":{"id_str":"1746900000000000001","created_at":"Mon Jan 15 09:30:00 +0000 2024","full_text":"Shipped the new parser today &amp; it is fast https://t.co/abc123 #rust","favorite_count":"42","retweet_count":"7","lang":"en","source":"<a href=\"https://x.com\" rel=\"nofollow\">Twitter Web App</a>","entities":{"hashtags":[{"text":"rust"}],"urls":[{"url":"https://t.co/abc123","expanded_url":"https://example.com/parser"}]}}},
  {"tweet":{"id_str":"1746900000000000002","created_at":"Mon Jan 15 11:00:00 +0000 2024","full_text":"@bob good point, will fix","favorite_count":"3","retweet_count":"0","lang":"en","in_reply_to_screen_name":"bob"}}
]
```

Output — summary plus transcript, plain text, top 1 by likes:

```text
Tweets in file: 2
Tweets after filters: 2
Tweets shown: 2 (1 original, 1 replies, 0 retweets)
Likes received: 45 (avg 22.50 per tweet)
Retweets received: 7 (avg 3.50 per tweet)
Words shown: 16
Date range: 2024-01-15 to 2024-01-15

Tweets per year:
  2024       2  100.00%

Top hashtags:
  #rust         1  50.00%

Top link domains:
  example.com       1  50.00%

Top 1 by likes:
  2024-01-15 — Shipped the new parser today & it is fast https://example.com/parser #rust  42 likes, 7 retweets  https://twitter.com/i/web/status/1746900000000000001

=== Transcript ===

[2024-01-15 11:00:00 UTC] reply · to @bob · 3 likes · 0 retweets · en
@bob good point, will fix
https://twitter.com/i/web/status/1746900000000000002

[2024-01-15 09:30:00 UTC] original · 42 likes · 7 retweets · en · Twitter Web App
Shipped the new parser today & it is fast https://example.com/parser #rust
https://twitter.com/i/web/status/1746900000000000001
```

Note what changed: `&amp;` became `&`, `https://t.co/abc123` became `https://example.com/parser`, `Twitter Web App` was unwrapped from its HTML anchor, and each tweet gained a permalink built from its id.

## Options

| Option | What it does |
| --- | --- |
| **Output** | Tweets only, summary only, or both. |
| **Format** | Markdown (headings, tables, linked permalinks), plain text, escaped HTML you can paste into a page, or CSV rows for a spreadsheet. |
| **Sort by** | Newest first (the timeline order), oldest first for a chronological read, most liked, or most retweeted. |
| **Search text** | Case-insensitive substring match against the tweet text, applied *after* `t.co` links are expanded — so searching for a destination domain works. |
| **From / To date** | Inclusive UTC bounds in `YYYY-MM-DD` form, compared against each tweet's UTC date. |
| **Include replies** | On by default. Off keeps standalone posts and retweets only. |
| **Include retweets** | On by default. Off keeps only what you wrote. |
| **Expand t.co links** | On by default. Off keeps the tweet text byte-for-byte as exported. |
| **Top tweets to list** | How many most-liked tweets appear in the summary. `0` skips that table. |
| **Max tweets** | Cap applied after filtering and sorting. `0` means no cap; use a small number to preview a huge archive. |

## Limits & edge cases

- **Paste the text, not the zip.** Extract the archive first and open `data/tweets.js` as text. A bare JSON array, un-enveloped tweet objects, and any `window.YTD.tweet.partN =` variant are all accepted; the trailing `;` is fine.
- **Multi-part archives.** Very large exports split into `tweets-part1.js`, `tweets-part2.js` and so on. Run each part separately, or paste the concatenated JSON arrays as one array.
- **`tweets.js` does not contain your handle.** Permalinks therefore use the account-agnostic `https://twitter.com/i/web/status/<id>` form, which redirects to the live post.
- **Everything is UTC.** `created_at` carries a `+0000` offset in the export; dates, the date filters and the per-year rows are all UTC, so results are reproducible regardless of your timezone.
- **Media is referenced, not embedded.** Photos and videos become `[photo: url]` / `[video: url]` placeholders built from the archive metadata. The tool never downloads files or touches the archive's `media/` folder.
- **Engagement counts are frozen at export time.** Likes and retweets are whatever Twitter/X wrote into the file, not live numbers. Retweets you made typically show `0` likes, since the counts belong to the original post.
- **Only `tweets.js`.** Direct messages, likes, followers and ad data live in separate files in the export and are not read here.
- **Unparseable timestamps fall back to the epoch**, so a tweet with a malformed `created_at` sorts to 1970 rather than disappearing. Empty input, non-JSON input, JSON that is not an array, an array with no tweet objects, and reversed date bounds are reported as explicit errors.
- **Big archives are browser-bound.** Tens of thousands of tweets parse fine, but rendering them all at once is heavy. Filter or cap first, then remove the cap for the final export.

## FAQ

<details>
<summary>Where do I find tweets.js?</summary>

Request your archive from Twitter/X (Settings → Your account → Download an archive), wait for the email, download the zip and extract it. The file is at `data/tweets.js`. Open it in a text editor and paste the whole thing — including the `window.YTD.tweets.part0 = [` first line.

</details>

<details>
<summary>Is my archive uploaded anywhere?</summary>

No. The parser is Rust compiled to WebAssembly and runs inside your browser tab. There is no upload, no account, no API call and no network request carrying your tweets. The same code runs offline through the CLI.

</details>

<details>
<summary>Why are my links shortened, and how do I get the real URLs?</summary>

Twitter rewrote every outbound link to a `t.co` redirect, but the archive also stores the original in `entities.urls[].expanded_url`. With **Expand t.co links** on (the default) each short link is swapped for that original, and the redundant `t.co` link that duplicates an attached photo is removed. Turn the option off if you want the text exactly as exported.

</details>

<details>
<summary>How does it tell an original from a reply or a retweet?</summary>

A tweet is a reply if it carries `in_reply_to_screen_name`, `in_reply_to_status_id_str` or `in_reply_to_user_id_str` — the transcript then shows `reply · to @name`. It is a retweet if it carries a `retweeted_status` or its text begins with `RT @`. Everything else is an original. Replies and retweets each have their own checkbox, so you can export only what you wrote.

</details>

<details>
<summary>Can I get the data into a spreadsheet?</summary>

Yes — choose **CSV rows**. The transcript becomes `date,id,kind,likes,retweets,language,source,text,permalink` with ISO-8601 UTC timestamps, and the summary becomes `section,label,value,share` rows tagged `summary`, `year`, `hashtag`, `mention`, `domain`, `source`, `language` or `top`. Cells containing commas, quotes or newlines are quoted.

</details>

<details>
<summary>Can it show my most popular tweets?</summary>

Yes, two ways. The summary lists the top N most-liked tweets with their engagement and permalinks (set **Top tweets to list**, or `0` to hide the table). For the whole transcript in popularity order, set **Sort by** to most liked or most retweeted — ties break by date, then by tweet id, so the order is stable.

</details>

<details>
<summary>Does it work on an old archive, or one from a third-party tool?</summary>

Usually. Unknown fields are ignored, so newer and older export shapes both read. Engagement counts are accepted as JSON strings (what the export writes) or as numbers (what many third-party dumps write), and timestamps are accepted in Twitter's `Mon Jan 15 09:30:00 +0000 2024` form or as ISO-8601. A missing field just renders as empty rather than failing the whole file.

</details>

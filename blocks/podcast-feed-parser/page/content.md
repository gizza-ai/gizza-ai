## About this tool

Paste a podcast RSS/XML feed and this tool extracts the channel metadata plus a clean episode list. It recognises common RSS 2.0 podcast fields, iTunes namespace fields, Atom enclosure links, publish dates, durations, GUIDs, episode links, and audio enclosure URLs.

Use it to inspect a feed, debug missing enclosure metadata, copy episode JSON into scripts, or compare podcast feed output during migrations. The `limit` and `order` options let you focus on the newest or oldest entries, and descriptions are optional so the default output stays compact.

Everything runs locally in your browser. The feed XML is not uploaded.

## FAQ

<details>
<summary>Can it fetch a feed from a URL, or do I have to paste the XML?</summary>

Paste the XML. The parser is fully local and does no network fetching — grab
the feed with your browser, `curl`, or your podcast host's export, then paste
it here. That is also why it works offline and why nothing is uploaded.

</details>

<details>
<summary>Which feed formats and fields does it understand?</summary>

RSS 2.0 (including the `itunes:` podcast namespace) and Atom 1.0. Per episode
it extracts the title, GUID, page link, audio enclosure (URL, MIME type, byte
size), season/episode numbers, the explicit flag, and — with the descriptions
option on — a plain-text summary. Empty fields are simply omitted from the
JSON rather than emitted as nulls.

</details>

<details>
<summary>Why do dates and durations look different from the raw feed?</summary>

They are normalised: RSS `pubDate` (RFC 2822) and Atom dates (RFC 3339) both
become an RFC 3339 `published` value, with the original preserved in
`published_raw`. Durations become `HH:MM:SS` in `duration` plus whole seconds
in `duration_seconds`, whether the feed wrote `3723`, `62:03`, or `1:02:03`.

</details>

<details>
<summary>How do the limit and order options interact?</summary>

`order` is applied first, then `limit` caps the list — so `order = "newest"`
with `limit = 10` gives the 10 most recent episodes by publish date, while the
default `order = "feed"` keeps the feed's own ordering and `limit = 0` returns
everything.

</details>

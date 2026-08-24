## About this tool

URL query strings are noisy. The same request can arrive as `?b=2&a=1`, `?a=1&b=2`, `?a=1&b=2&utm_source=news`, or `?q=hello+world` versus `?q=hello%20world`. They usually point at the same resource, but caches, logs, redirects, analytics exports, and review diffs treat them as different strings.

This tool canonicalizes only the query string. It sorts parameters, collapses duplicates, rewrites percent-encoding to one spelling, and can remove tracking parameters or custom allow/deny lists. The scheme, host, port, path, and fragment are copied through byte-for-byte, so it pairs cleanly with path-focused tools such as trailing-slash normalizers.

A worked example with the defaults — sort by key, drop exact duplicate pairs, normalize encoding, write spaces as `%20`, keep tracking params:

```
https://example.com/p?utm_source=news&b=hello+world&a=1&b=hello%20world
https://example.com/search?q=caf%c3%a9&page=2&page=2
b=2&a=1
```

becomes

```
https://example.com/p?a=1&b=hello%20world&utm_source=news
https://example.com/search?page=2&q=caf%C3%A9
a=1&b=2
```

Turn on **Strip tracking parameters** to remove `utm_*`, `fbclid`, `gclid`, `msclkid`, HubSpot, Matomo/Piwik, Mailchimp, and similar click IDs. Use **Also drop these names** for site-specific noise such as `sid,ref,session_id`; use **Keep only these names** when a cache key should contain only a few meaningful params such as `page,sort`. Both lists are comma-separated and support a trailing `*` prefix wildcard such as `x_*`.

**Duplicate parameters** defaults to **exact** because repeated keys can be meaningful: `tag=a&tag=b` is not the same as a duplicate. Choose **first** or **last** only when you know the endpoint treats repeated keys as overwrites.

**Result** can return all normalized URLs, only changed lines, a per-line CSV report, or a compact CSV summary. That makes the same tool useful for redirect maps, cache-key audits, and cleaning a pasted list before sharing.

Limits and edge cases:

- Up to 20,000 non-blank lines or 1,000,000 input bytes per run. Blank lines are ignored.
- Bare query strings such as `b=2&a=1` are accepted and returned without a leading `?`.
- Lines with no query string pass through unchanged.
- Fragments are preserved after the normalized query: `#section` stays `#section`.
- Malformed percent escapes are not fatal. A literal stray `%` is emitted as `%25`.
- This is not a full URL canonicalizer. It does not lowercase hosts, remove default ports, resolve `..` path segments, or change trailing slashes.

## FAQ

<details>
<summary>Can sorting query parameters change what a server returns?</summary>

Usually no: query parameters are normally treated as an unordered map or a multimap. But a few APIs do care about order, especially signed URLs, old CGI handlers, and endpoints that treat repeated keys as a sequence. If order matters for your endpoint, set **Parameter order** to **Keep the original order** and use the tool only for encoding cleanup, filtering, or reporting.

</details>

<details>
<summary>Why is duplicate handling set to exact by default?</summary>

Because repeated keys are often intentional. Filters, tags, and batch IDs commonly use `tag=a&tag=b` or `id=1&id=2`. Dropping every later value would silently change the request. The **exact** default removes only repeated name/value pairs that are truly identical after encoding normalization. If your application uses first-wins or last-wins semantics, choose that mode explicitly.

</details>

<details>
<summary>What happens to plus signs and spaces?</summary>

In URL query strings, a literal `+` is conventionally read as a space. The tool follows that convention: `q=hello+world` and `q=hello%20world` converge. The **Spaces are written as** control chooses the output spelling, `%20` or `+`. A real plus sign must be written as `%2B`, and it stays `%2B` after normalization.

</details>

<details>
<summary>Does this remove tracking parameters automatically?</summary>

No. Tracking removal is useful, but it is a separate decision from canonicalization, so the checkbox is off by default. Turn it on for shareable links or privacy cleanup. For business-specific parameters, add exact names or prefix rules in **Also drop these names**; for a strict cache key, prefer **Keep only these names**.

</details>

<details>
<summary>Is any URL sent to a server?</summary>

No. The browser page runs the same Rust/WASM code locally, and the CLI runs locally too. Pasted URLs are not fetched, followed, validated against the network, or uploaded. The tool only rewrites the text you give it.

</details>

## About this tool

URL lists pick up accidental differences everywhere: uppercase hosts, `:80` or `:443` default ports, `/a/../b` path segments, lowercase percent escapes, query parameters in random order, and tracking IDs pasted from campaigns. Those strings may point at the same resource, but caches, crawl exports, redirect maps, and code reviews see different text.

This tool applies syntax-based URL normalization to the whole URL. With the defaults it lowercases the scheme and host, removes a port the scheme already implies, resolves `.` and `..` path segments, rewrites percent-encoding to the RFC 3986 canonical spelling, and sorts query parameters by key. Optional controls add SEO cleanup on top: force `https`, add or strip `www.`, remove directory index files such as `index.html`, adjust trailing slashes, drop tracking parameters, remove fragments, and deduplicate a batch after normalization.

A worked example with the defaults:

```
HTTP://Example.COM:80/a/b/../c?b=2&a=1
https://example.com/search?q=caf%c3%a9&page=2&page=2
```

becomes

```
http://example.com/a/c?a=1&b=2
https://example.com/search?page=2&page=2&q=caf%C3%A9
```

For a stricter canonical link, choose **Force https**, **Strip leading www.**, **Remove directory index files**, **Remove fragments**, and **Strip tracking parameters**. For servers where paths are case-insensitive or logs contain repeated accidental slashes, the path-only options are available but off by default because those rewrites can change the addressed resource on a case-sensitive web server.

The **Base URL** field resolves relative references using RFC 3986 rules. For example, with base `https://example.com/docs/guide/index.html`, `../images/logo.png` becomes `https://example.com/docs/images/logo.png` and `?print=1&b=2&a=1` becomes `https://example.com/docs/guide/index.html?a=1&b=2&print=1`.

**Result** can return every normalized URL, only changed URLs, a per-line CSV report, or a compact CSV summary. That makes the same normalizer useful for crawl deduplication, redirect planning, cache-key audits, and preparing pasted URL lists before sharing.

Limits and edge cases:

- Up to 20,000 non-blank URLs or 1,000,000 input bytes per run.
- Absolute URLs, scheme-relative URLs, bare hosts, and relative references with a base URL are accepted.
- Blank lines are ignored.
- Invalid lines are kept by default so annotated lists survive a round trip; choose **Stop with an error** for strict validation.
- Query sorting does not understand application semantics. Signed URLs and APIs that depend on parameter order should use **Keep original order**.
- Tracking-parameter removal covers common analytics and click-ID names; use the dedicated query normalizer when you need custom allow/deny lists.
- No URL is fetched or checked on the network. The tool rewrites text only.

## FAQ

<details>
<summary>Can URL normalization change what a server returns?</summary>

Yes, if you enable a rewrite that is not safe for your server. Lowercasing the host, dropping default ports, sorting ordinary query parameters, and resolving dot-segments are usually safe syntax normalizations. Lowercasing a path, collapsing repeated slashes, stripping `www.`, removing fragments, deleting tracking parameters, and changing trailing slashes are policy decisions. They are exposed as explicit controls so you can match the rules of the site you are auditing.

</details>

<details>
<summary>Why are path lowercase and repeated-slash cleanup off by default?</summary>

Paths are case-sensitive on many web servers, and an empty path segment can be meaningful to some routers. That means `/Docs/Page` and `/docs/page`, or `/a//b` and `/a/b`, are not guaranteed to be the same URL. Turn those options on only for a host where you know the server treats those spellings identically.

</details>

<details>
<summary>What happens to tracking parameters?</summary>

Tracking removal is optional and off by default. When enabled, the tool drops common analytics and click-ID parameters such as `utm_*`, `fbclid`, `gclid`, `msclkid`, `yclid`, HubSpot, Matomo/Piwik, Mailchimp, and similar families. Other query parameters are preserved, sorted, and percent-encoded according to the selected options.

</details>

<details>
<summary>How are relative URLs resolved?</summary>

If **Base URL** is blank, a relative line stays relative. If you provide an absolute base such as `https://example.com/docs/guide/index.html`, the tool resolves each relative reference against that base before the rest of normalization runs. Lines that already include their own scheme keep their own origin.

</details>

<details>
<summary>Is any URL sent to a server?</summary>

No. The browser page runs the Rust/WASM normalizer locally, and the CLI runs locally too. Pasted URLs are not fetched, followed, validated against DNS, or uploaded. The output is only a rewritten version of the text you supplied.

</details>

## About this tool

HTTP Headers Diff compares two pasted header blocks and reports what changed from the first set to the second: added headers, removed headers, changed values, and unchanged names. It is useful for release checks, CDN/proxy debugging, security-header reviews, cache-policy changes, and comparing request or response captures.

Header names are matched case-insensitively, repeated headers are combined according to HTTP header-list rules, and `Set-Cookie` is kept as separate lines instead of being comma-joined. Optional request/status lines such as `GET / HTTP/1.1` or `HTTP/1.1 200 OK` are skipped so you can paste raw captures.

## Worked example

First headers:

```text
Content-Type: text/html
Server: nginx
X-Frame-Options: DENY
```

Second headers:

```text
Content-Type: application/json
Server: nginx
Cache-Control: no-cache
```

Output:

```text
Header diff — 1 added, 1 removed, 1 changed, 1 unchanged

Added (1):
  + Cache-Control: no-cache

Removed (1):
  - X-Frame-Options: DENY

Changed (1):
  ~ Content-Type: text/html -> application/json

Unchanged (1):
  Server
```

Use **Ignore headers** for noisy values such as `Date`, `Age`, or request IDs. Turn on **Ignore token order** when comparing comma-list headers like `Cache-Control` or `Vary` where only the ordering changed.

## Limits & edge cases

- This is a paste-and-compare tool; it does not fetch live URLs or execute curl commands.
- The diff direction is first/left/old → second/right/new.
- Header names are case-insensitive, but values are compared exactly unless **Ignore token order** is enabled.
- Repeated headers are joined with `, ` except `Set-Cookie`, which stays newline-separated.
- Obsolete folded continuation lines are accepted and folded into the previous header value.
- The parser stops at the first blank line, treating anything after it as a message body and ignoring it.

## FAQ

<details>
<summary>Can I paste a full HTTP response?</summary>

Yes, if the first line is a status line such as `HTTP/1.1 200 OK`. The start line is skipped and only the following headers are compared. The parser stops at the first blank line, so a response body pasted after the headers is ignored.

</details>

<details>
<summary>Are header names case-sensitive?</summary>

No. HTTP header names are matched case-insensitively, so `content-type` and `Content-Type` are the same header. The report displays canonical title-case names for readability.

</details>

<details>
<summary>How are repeated headers handled?</summary>

Repeated headers are combined before comparison, which matches normal HTTP list-header behavior. `Set-Cookie` is the exception: each cookie stays on its own line because cookie values must not be comma-joined.

</details>

<details>
<summary>Why would I ignore token order?</summary>

Some headers are comma-separated lists. `Cache-Control: no-cache, no-store` and `Cache-Control: no-store, no-cache` usually mean the same thing. Enable **Ignore token order** to treat those reorder-only changes as unchanged.

</details>

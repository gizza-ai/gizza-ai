## About this tool

HTTP field names are case-insensitive on the wire, but every log, diff, and string comparison treats them as plain text. So the same request shows up as `Content-Type`, `content-type`, and `CONTENT-TYPE`; values arrive with ragged spacing after the colon; repeated names appear in a different order in every capture. Comparing two of those blocks by eye is miserable.

This tool rewrites a pasted header block into one canonical form. Names are re-cased, values are trimmed, obsolete indented continuation lines are joined, repeated names are folded, and the lines are sorted, so two captures of the same request line up.

A worked example with the defaults — canonical casing, sorted by name, duplicates joined, folds unwrapped:

```
GET /v1/items?page=2 HTTP/1.1
host:   api.example.com
ACCEPT: application/json
accept: text/plain
x-request-id:   9f3c
user-agent: demo/1.0
content-type:application/json
```

becomes

```
GET /v1/items?page=2 HTTP/1.1
Accept: application/json, text/plain
Content-Type: application/json
Host: api.example.com
User-Agent: demo/1.0
X-Request-ID: 9f3c
```

The optional request line at the top is kept verbatim, `x-request-id` gets its conventional `X-Request-ID` spelling from the built-in exception table, and the two `accept` lines merge into one comma-joined field.

**Field name casing** offers canonical `Title-Case`, `lowercase` (what HTTP/2 and most proxy logs use), `UPPERCASE`, or preserving whatever casing each name had the first time it appeared. Canonical mode knows the names that plain title-casing gets wrong: `ETag`, `WWW-Authenticate`, `DNT`, `TE`, `Content-MD5`, `X-XSS-Protection`, `Sec-WebSocket-Key`, `X-UA-Compatible`, `Last-Event-ID`, and similar.

**Drop these names** and **Keep only these names** take comma-separated lists with an optional trailing `*` prefix wildcard, so `authorization,cookie,x-*` redacts credentials and internal headers before a capture goes into a bug report, and `host,content-type` reduces a block to the two fields you actually want to compare.

**Result** can be the normalized block, the same headers as copy-pasteable `curl -H` flags, or a CSV summary of what changed.

Limits and edge cases:

- Up to 20,000 lines or 1,000,000 bytes per run.
- The first blank line ends the header block, so a pasted body is ignored; the summary counts those skipped lines.
- `Set-Cookie` is never comma-joined, even in the default combine mode — RFC 6265 forbids it, so each cookie stays on its own line.
- HTTP/2 pseudo-headers (`:method`, `:path`, `:scheme`, `:authority`, `:status`) stay lowercase in every casing mode and sort before regular fields. They are skipped in `curl` output, because curl derives them from the URL and method.
- A line without a colon, an empty field name, or a name containing characters outside the RFC 7230 token set is an error that names the offending line.
- Values are trimmed, not rewritten. Quoting, cookie syntax, dates, and encodings are passed through unchanged.
- Structured `name → value` JSON is not an output here — that is the separate HTTP header parser tool.

## FAQ

<details>
<summary>Does changing header name casing change the request?</summary>

No. RFC 7230 (HTTP/1.1) defines field names as case-insensitive, so `Content-Type`, `content-type`, and `CONTENT-TYPE` are the same header to any conformant server. Casing matters only to your own tooling: string comparisons, diffs, log greps, and code that indexes a header map. HTTP/2 and HTTP/3 go further and require lowercase names on the wire, which is why `lowercase` is offered as a mode and why pseudo-headers are always emitted lowercase.

</details>

<details>
<summary>Why are duplicate headers joined with a comma by default?</summary>

Because that is what the specification says a recipient may do: a field that appears more than once can be combined into one field whose value is the values joined with a comma, in the order they arrived. That makes the block canonical and diff-friendly. The exception is `Set-Cookie`, which must never be combined, so it always stays one line per cookie. If your parser treats repeats as first-wins or last-wins, pick that mode explicitly, or choose the list mode to keep every occurrence on its own line.

</details>

<details>
<summary>What is an indented continuation line?</summary>

Old HTTP allowed a long field value to be wrapped onto extra lines that begin with a space or a tab — line folding. It is deprecated, but it still shows up in captures, mail-style dumps, and hand-written fixtures. With **Join indented continuation lines** on, the wrapped text is appended to the header above it with a single space, which is how a modern parser reads it. Turn it off to keep the fold as an indented second line. A continuation with no header in front of it is an error either way, since there is nothing to attach it to.

</details>

<details>
<summary>Can I use this to redact an Authorization header before sharing a capture?</summary>

Yes, and that is a common use. Put the sensitive names in **Drop these names** — for example `authorization,cookie,set-cookie,x-api-key` — or use a prefix rule such as `x-internal-*`. The matching is case-insensitive, so you do not have to guess how the header was spelled. Use **Keep only these names** for the opposite approach: list the few fields worth sharing and everything else disappears. Note that the values are removed, not masked, so nothing sensitive remains in the output.

</details>

<details>
<summary>Is anything sent to a server?</summary>

No. The page runs the same Rust compiled to WebAssembly directly in your browser, and the command-line version runs locally. Pasted headers are not uploaded, logged, or used to make a request — the tool only rewrites the text you give it. That matters here, because header blocks routinely contain cookies and bearer tokens.

</details>

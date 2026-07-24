## About this tool

Paste a raw HTTP request or response header block and this tool turns it into a
clean, structured JSON map of `name → value`. It is built for the moment you
copy a jumble of headers out of your browser's network panel, a `curl -i`
dump, or a log line and want them as tidy, comparable data instead of loose
text.

Header names are matched **case-insensitively** (HTTP treats `Content-Type`,
`content-type`, and `CONTENT-TYPE` as the same field) and then re-cased to the
style you pick: canonical Title-Case like `Content-Type`, all `lowercase`, all
`UPPERCASE`, or the original casing exactly as you pasted it. Repeated headers
are folded by a policy you choose — combine them into one comma-joined value
per RFC 7230, keep every occurrence as a JSON array, or keep just the first or
last. `Set-Cookie` is never comma-joined (RFC 6265 forbids it), so under the
combine policy it always stays a list.

An optional leading request line (`GET /path HTTP/1.1`) or status line
(`HTTP/1.1 200 OK`) is detected and captured separately, so you can paste a
whole response head without hand-trimming it first. Parsing stops at the first
blank line — the head/body boundary — so a pasted body is ignored. The result
also reports the detected message kind, the number of distinct headers, the raw
line count, and which header names were duplicated.

Everything runs locally in your browser through a small WebAssembly module.
Nothing is uploaded, so it is safe to paste headers that carry cookies, tokens,
or other sensitive values.

### Example

Paste this response head:

```
HTTP/1.1 200 OK
Content-Type: text/html; charset=utf-8
Cache-Control: max-age=60
Set-Cookie: id=1; Path=/
Set-Cookie: sess=xyz; HttpOnly
```

With canonical casing and the combine policy you get:

```json
{
  "kind": "response",
  "start_line": "HTTP/1.1 200 OK",
  "case": "canonical",
  "duplicate_policy": "combine",
  "headers": {
    "Content-Type": "text/html; charset=utf-8",
    "Cache-Control": "max-age=60",
    "Set-Cookie": ["id=1; Path=/", "sess=xyz; HttpOnly"]
  },
  "count": 3,
  "line_count": 4,
  "duplicates": ["Set-Cookie"]
}
```

## FAQ

<details>
<summary>How does it handle repeated headers like Set-Cookie?</summary>

You choose the policy. **Combine** (the default) joins repeated values with
`, ` as allowed by RFC 7230 — except `Set-Cookie`, which stays a JSON array
because RFC 6265 forbids folding cookies onto one line. **List** keeps every
header as an array of all its values. **First** and **last** keep only that one
occurrence. Any name that appeared more than once is also listed in the
`duplicates` field.

</details>

<details>
<summary>Do I need to strip the request or status line first?</summary>

No. A leading request line such as `GET /path HTTP/1.1` or a status line such
as `HTTP/1.1 200 OK` is detected automatically, reported in `kind`, and
returned separately in `start_line`. If the block has no start line the `kind`
is simply `headers`. Paste whatever you copied.

</details>

<details>
<summary>What does the casing option change?</summary>

Only how names are written in the output — matching is always
case-insensitive. **Canonical** renders standard HTTP Title-Case
(`content-type` → `Content-Type`, with special cases like `ETag` and
`WWW-Authenticate`). **lower** and **upper** force a single case, which is handy
for diffing two header sets. **original** keeps the exact casing you pasted for
the first occurrence of each name.

</details>

<details>
<summary>Can I paste a full response including the body?</summary>

Yes. Parsing follows the HTTP rule and stops at the first blank line, which
separates the head from the body, so anything after it is ignored. It also
accepts both CRLF and bare-LF line endings and joins obsolete folded
continuation lines (a line that begins with a space or tab) onto the header
above them.

</details>

<details>
<summary>Is my data sent anywhere?</summary>

No. The parser is a WebAssembly module that runs entirely in your browser. The
header text never leaves your machine, so pasting cookies, authorization
tokens, or other sensitive header values is safe.

</details>

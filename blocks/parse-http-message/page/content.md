## About this tool

**HTTP Message Parser** decodes a raw HTTP/1.x message — a **request** or a
**response** — into its structured parts:

- **Start line** — for a request, the **method**, **request target**, plus the
  **path** and **query** split out; for a response, the **status code**, the
  **reason phrase** (filled in from the code when omitted), and the **status
  class** (Informational / Success / Redirection / Client Error / Server Error).
- **HTTP version** — e.g. `HTTP/1.1`.
- **Headers** — every header line in **wire order**, with **duplicates
  preserved** (so repeated `Set-Cookie` lines are not collapsed). Obsolete
  line-folded headers are joined back together.
- **Convenience fields** — the **Content-Type**, the parsed **Content-Length**,
  and whether `Transfer-Encoding: chunked` is set.
- **Body** — the bytes after the blank line, with their length.

Request vs response is detected automatically: a status line starts with
`HTTP/`, a request line ends with it. Both `CRLF` and bare-`LF` line endings are
accepted, so you can paste a message copied from logs or an editor that
normalised the newlines.

### Example

```
GET /search?q=cats&page=2 HTTP/1.1
Host: example.com
Accept: text/html
Accept-Language: en-US

```

decodes to method `GET`, path `/search`, query `q=cats&page=2`, three headers,
and an empty body.

### Common uses

- Inspect a request or response captured in your proxy, server log, or DevTools.
- Confirm exactly which headers (and how many duplicates) a client or server
  sent without re-running the request.
- Pull the status, Content-Type, and body out of a saved HTTP exchange.

## FAQ

<details>
<summary>How does it know whether I pasted a request or a response?</summary>

By the first token of the start line: a **response** begins with the version
(`HTTP/1.1 200 OK`), while a **request** ends with it (`GET / HTTP/1.1`). The
detection is automatic, and a start line that fits neither shape is reported
as an error rather than parsed into nonsense.

</details>

<details>
<summary>Are repeated headers like Set-Cookie collapsed together?</summary>

No — every header line is kept in **wire order with duplicates preserved**, so
three `Set-Cookie` lines show up as three entries. Obsolete line-folded
headers (a continuation line starting with a space) are the one exception:
they're joined back onto the header they continue, per the HTTP spec.

</details>

<details>
<summary>Do I need real CRLF line endings?</summary>

No. Both `CRLF` and bare `LF` are accepted, so a message copied from a log
file, editor or terminal that normalised the newlines parses fine. The
head/body boundary is the first blank line either way.

</details>

<details>
<summary>Does it decode a chunked or gzip-encoded body?</summary>

It reports the raw body after the blank line and flags when
`Transfer-Encoding: chunked` is present, but it does **not** de-chunk or
decompress the payload — you get the bytes exactly as pasted, plus the parsed
Content-Type and Content-Length for reference.

</details>

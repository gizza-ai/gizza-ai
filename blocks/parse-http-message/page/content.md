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

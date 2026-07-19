## About this tool

**HAR Body Stripper** removes the payload data from a **HAR (HTTP Archive)**
capture — the JSON file browser DevTools export from the Network tab via
*Save all as HAR*. HAR files balloon because every response body (HTML,
JavaScript, base64-encoded images and fonts) is stored inline, and they leak
because request bodies carry form fields, passwords, and JSON uploads. This
tool deletes exactly those payloads and nothing else:

- **Request bodies** — `postData.text` and `postData.params` (form fields,
  JSON/GraphQL uploads).
- **Response bodies** — `content.text` and its `encoding` marker.
- **Websocket payloads** — the `data` of Chrome's `_webSocketMessages`
  frames, a common sanitizer blind spot (`send` frames follow the request
  side, `receive` frames the response side).

Everything else survives untouched: URLs, methods, status codes, **headers,
cookies**, timings, `content.size`, `mimeType`, and `bodySize` — so the
stripped capture still loads in any HAR viewer, waterfalls still render, and
size analysis still works. Key order is preserved, which keeps the output
diff-able against the original.

### Choosing what to strip

- **Strip** — both sides (default), request bodies only, or response bodies
  only.
- **Only mime types** — comma-separated mimeType substrings such as
  `image/,font/,video/`: strip just the binary bulk and keep API JSON
  readable. Bodies with no recorded mimeType (and websocket frames, which
  have none) are kept whenever a filter is set.
- **Min body size** — only strip bodies at least this many bytes, e.g.
  `10240` keeps small API responses but drops big blobs. Response bodies
  measure the recorded decoded `content.size` (falling back to the stored
  text length); request bodies measure the stored `postData` length.
- **Output** — the stripped HAR as JSON (ready to save as a `.har` file —
  use the Download link), or a **dry-run summary** of what would be removed.
- **Pretty-print** — off by default: compact single-line JSON shrinks the
  file the most, since DevTools exports are pretty-printed.

### Worked example

A two-entry capture — a login POST with a JSON body, and a base64-inlined
image response:

```json
{
  "log": {
    "version": "1.2",
    "creator": { "name": "devtools", "version": "1" },
    "entries": [
      {
        "startedDateTime": "2024-01-01T00:00:00.000Z",
        "time": 120,
        "request": {
          "method": "POST",
          "url": "https://example.com/api/login",
          "postData": { "mimeType": "application/json", "text": "{\"user\":\"alice\",\"password\":\"hunter2\"}" }
        },
        "response": {
          "status": 200,
          "statusText": "OK",
          "content": { "size": 20, "mimeType": "application/json", "text": "{\"session\":\"abc123\"}" },
          "bodySize": 20
        }
      },
      {
        "startedDateTime": "2024-01-01T00:00:01.000Z",
        "time": 80,
        "request": { "method": "GET", "url": "https://example.com/logo.png" },
        "response": {
          "status": 200,
          "statusText": "OK",
          "content": { "size": 51200, "mimeType": "image/png", "encoding": "base64", "text": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==" },
          "bodySize": 51234
        }
      }
    ]
  }
}
```

strips (with the defaults) to this compact HAR — the password, session token,
and image data are gone; sizes, mime types, and timings remain:

```json
{"log":{"version":"1.2","creator":{"name":"devtools","version":"1"},"entries":[{"startedDateTime":"2024-01-01T00:00:00.000Z","time":120,"request":{"method":"POST","url":"https://example.com/api/login","postData":{"mimeType":"application/json"}},"response":{"status":200,"statusText":"OK","content":{"size":20,"mimeType":"application/json"},"bodySize":20}},{"startedDateTime":"2024-01-01T00:00:01.000Z","time":80,"request":{"method":"GET","url":"https://example.com/logo.png"},"response":{"status":200,"statusText":"OK","content":{"size":51200,"mimeType":"image/png"},"bodySize":51234}}]}}
```

The same run with **Output = Summary (dry run)** reports instead:

```text
HAR body strip summary
entries scanned: 2
request bodies stripped: 1 (37 B)
response bodies stripped: 2 (116 B)
websocket payloads stripped: 0
size: 1.1 KB → 588 B (48.1% smaller)
Run with output=har to get the stripped capture.
```

On a real capture the shrink is far bigger — inline bodies are usually 90%+
of a HAR's bulk.

### Limits and edge cases

- **Bodies only.** Cookies, `Authorization` headers, and query-string tokens
  are **not** redacted — pair this with a redaction step before sharing a
  capture whose headers are sensitive. URLs can also carry secrets.
- Up to **10,000 entries** per run; larger captures are rejected with a
  clear error rather than truncated.
- Entries that are malformed or have no body are skipped gracefully; only
  non-JSON input, or JSON without a `log.entries` array, is an error.
- The summary's stripped-bytes figures count the **stored** text removed
  from the file; the size threshold compares the recorded decoded
  `content.size` where the exporter provided one.
- Everything runs in your browser's WebAssembly — the capture is never
  uploaded.

### Handy for

- Getting a HAR under a support portal's upload limit without losing the
  request waterfall.
- Clearing payload data (login forms, API responses, websocket traffic) out
  of a capture before attaching it to a bug report.
- Keeping API JSON readable while dumping the image/font/video bulk
  (`image/,font/,video/` mime filter).

## FAQ

<details>
<summary>How much smaller will my HAR file get?</summary>

It depends on how much inline body data the capture holds — typically most
of it. Response bodies (HTML, scripts, base64-encoded images and fonts) are
usually 90% or more of a HAR's bytes, and compact re-serialization removes
the export's pretty-printing on top. Run with **Output = Summary** first to
see the exact before → after size without producing the file.

</details>

<details>
<summary>Is the stripped file safe to share?</summary>

Safer, not safe. All request payloads (form fields, JSON uploads), response
bodies, and websocket payloads are gone — but this tool deliberately does
**not** touch cookies, `Authorization`/`Set-Cookie` headers, or tokens in
URL query strings, so the capture stays maximally analyzable. Review or
redact those separately before sharing a capture from an authenticated
session. Processing runs entirely in your browser; nothing is uploaded.

</details>

<details>
<summary>Will the stripped HAR still open in DevTools and HAR viewers?</summary>

Yes. Only optional body fields (`postData.text`/`params`, `content.text`,
`encoding`, websocket `data`) are removed — exactly the fields browsers
already omit when you export without content. Structure, key order, sizes,
mime types, headers, and timings are preserved, so viewers render the full
request list and waterfall; only the response preview panes are empty.

</details>

<details>
<summary>Can I keep small API responses and only drop the big binary bodies?</summary>

Yes, two ways that combine: set **Only mime types** to
`image/,font/,video/` to strip just those content types, or set **Min body
size** (e.g. `10240`) so bodies under 10 KB survive. With either filter
active the JSON your API returned stays readable while the bulk disappears.

</details>

<details>
<summary>Why is the output on one line, and can I get readable JSON?</summary>

Compact output is the point — a single-line file with no indentation is the
smallest faithful serialization, and DevTools exports waste a lot of bytes
on pretty-printing. Tick **Pretty-print output** if you want a 2-space
indented file for reading or diffing; it holds exactly the same data in a
larger file.

</details>

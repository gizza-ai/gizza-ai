## About this tool

**HAR Request Extractor** pulls the request list out of a **HAR (HTTP
Archive)** capture — the JSON file browser DevTools export from the Network
tab via *Save all as HAR*. Instead of scrolling a 40 MB JSON blob, you get one
line per request:

- **Method + URL** — what was requested.
- **Status** — the HTTP status code; failed requests that never got a
  response are listed with status `0`.
- **Type** — the response `mimeType`, with `; charset=…` parameters stripped.
- **Size** — bytes on the wire: `response.bodySize` when the exporter
  recorded it, else Chrome's `_transferSize`, else the decoded
  `content.size`. Unknown sizes show as `-`.
- **Time** — the entry's total time in milliseconds.

The **#** column is each request's position in the original capture, so a row
stays findable in DevTools even after filtering or sorting.

### Filters and sorting

- **Status filter:** `2xx`, `3xx`, `4xx`, `5xx`, or **errors** (4xx + 5xx +
  failed status-0 requests).
- **Method filter:** case-insensitive exact match (`GET`, `POST`, …).
- **URL contains:** case-insensitive substring, e.g. `/api/`, `.png`, or a
  hostname.
- **Sort:** capture order, slowest first, or largest first.

### Output formats

- **Table** — aligned text columns plus a summary line
  (`n of m requests · X transferred`; the byte count sums the listed rows).
- **CSV** — header + one row per request
  (`index,method,url,status,status_text,mime_type,size_bytes,time_ms,started`),
  ready for a spreadsheet.
- **JSON** — an array of request objects with the same fields (unknown values
  are `null`).
- **URL list** — one URL per line, in row order, ready to pipe.

### Worked example

A three-request capture (an HTML page, a slow API call, a missing image):

```json
{"log":{"entries":[
  {"startedDateTime":"2024-01-01T00:00:00.000Z","time":102.5,
   "request":{"method":"GET","url":"https://example.com/"},
   "response":{"status":200,"statusText":"OK","content":{"mimeType":"text/html","size":5120},"bodySize":2048}},
  {"startedDateTime":"2024-01-01T00:00:01.000Z","time":812,
   "request":{"method":"POST","url":"https://example.com/api/search"},
   "response":{"status":200,"statusText":"OK","content":{"mimeType":"application/json","size":20480},"bodySize":-1,"_transferSize":10240}},
  {"startedDateTime":"2024-01-01T00:00:02.000Z","time":54,
   "request":{"method":"GET","url":"https://cdn.example.com/logo.png"},
   "response":{"status":404,"statusText":"Not Found","content":{"mimeType":"image/png","size":0},"bodySize":512}}
]}}
```

extracts to this table:

```text
3 of 3 requests · 12.5 KB transferred

#  METHOD  STATUS  TYPE              SIZE     TIME    URL
1  GET     200     text/html         2.0 KB   103 ms  https://example.com/
2  POST    200     application/json  10.0 KB  812 ms  https://example.com/api/search
3  GET     404     image/png         512 B    54 ms   https://cdn.example.com/logo.png
```

With the status filter on **Errors**, only row 3 remains; with **Slowest
first**, the 812 ms API call moves to the top (and keeps its `#2`).

### Limits and edge cases

- Only the request LIST is extracted — request/response **headers, cookies,
  and bodies are never read into the output**, so the result is safe to share
  even when the capture itself isn't.
- The extractor is deliberately forgiving: entries missing fields still list,
  with `-` (table/CSV) or `null` (JSON) for the unknowns. Only non-JSON input
  or JSON without a `log.entries` array is an error.
- Times are rounded to whole milliseconds in the table; CSV/JSON keep the
  exact values. Sizes in the table are 1024-based (`KB`/`MB`).
- Everything runs in one pass in your browser's WebAssembly — captures of
  tens of thousands of entries are fine; a multi-hundred-MB capture is
  limited by your browser's memory, not by an upload cap.

### Handy for

- Turning a bug-report HAR into a shareable request summary without leaking
  cookies or auth headers.
- Finding the slowest or heaviest requests behind a slow page load.
- Pulling every API call (`/api/`, `graphql`, …) out of a session as CSV for
  a spreadsheet or JSON for a script.

## FAQ

<details>
<summary>How do I get a HAR file in the first place?</summary>

Open your browser's DevTools (F12), go to the **Network** tab, reload the
page (or perform the action you want to capture), then right-click the
request list — or use the download icon — and choose **Save all as HAR**.
Chrome, Edge, Firefox, and Safari all export the same JSON format, as do
proxies like Charles, Fiddler, and mitmproxy.

</details>

<details>
<summary>Is it safe to paste a HAR here? They contain cookies and tokens.</summary>

The capture never leaves your machine — extraction runs entirely in your
browser via WebAssembly, with no upload. The output is also much safer than
the raw file: only method, URL, status, content type, size, and timing are
emitted; headers, cookies, and request/response bodies are never copied into
the result. URLs can still carry secrets in query strings, so skim the list
before sharing it.

</details>

<details>
<summary>Why does a request show status 0?</summary>

Status `0` means the request never received an HTTP response — it was
blocked, cancelled, timed out, or failed at the network level (DNS, TLS,
CORS). Browsers record such entries in the HAR with `status: 0`. The
**Errors** filter includes them alongside 4xx and 5xx responses.

</details>

<details>
<summary>Which size does the SIZE column show?</summary>

Bytes over the wire when available: the spec's `response.bodySize` if it's
non-negative, else Chrome's `_transferSize` extension field, else the decoded
`content.size`. HAR writers use `-1` for "unknown", and cached responses
often report `0` — rows where no size was recorded show `-` (or `null` in
JSON), and the summary line only sums the sizes that are known.

</details>

<details>
<summary>Can it show the full waterfall, headers, or response bodies?</summary>

No — this tool extracts the request *list*. Per-phase timing waterfalls
(DNS, connect, TLS, wait), header inspection, and body viewing are a
graphical HAR viewer's job. If you need to check whether a capture is
spec-complete instead, use a HAR validator; this extractor deliberately
tolerates incomplete entries.

</details>

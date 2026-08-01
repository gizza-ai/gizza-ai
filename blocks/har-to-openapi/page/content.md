## About this HAR to OpenAPI converter

This tool turns a **HAR (HTTP Archive)** capture — the network log your browser's
DevTools can export — into a draft **OpenAPI 3.x** specification, entirely in your
browser. Point it at the traffic an app or SPA actually made and get a starting
`openapi.yaml` (or `openapi.json`) describing the endpoints it hit, without writing
the spec by hand.

It reads `log.entries`, groups the requests by path and HTTP method, derives the
`servers` base URL from the request origins, and infers a JSON Schema for each
request body and for each response status code from the captured JSON. Id-like path
segments are collapsed into `{param}` templates so `/users/1` and `/users/2` become a
single `/users/{user}` operation.

### How to capture a HAR

1. Open your browser's **DevTools → Network** tab.
2. Reload the page or click through the flows you want documented.
3. Right-click any request → **Save all as HAR** (or use the download/export button).
4. Paste the saved JSON into the box above.

### What it infers

- **Paths & methods** — one operation per `(path, method)` seen in the capture.
- **Servers** — the distinct `scheme://host` origins become `servers` entries.
- **Path parameters** — numeric ids, UUIDs, and long opaque tokens are templated
  into `{param}` and named after the preceding collection segment (`/users/{user}`).
- **Query parameters** — collected from the URL and the HAR `queryString` array.
- **Request bodies** — JSON post data becomes a `requestBody` schema.
- **Responses** — one entry per observed status code, each with a schema (and, for
  JSON, an example) inferred from the captured body.

### Options

- **Output format** — YAML (default) or JSON. Both encode the same document.
- **OpenAPI version** — stamp `3.0.3` or `3.1.0`.
- **Parameterize id-like path segments** — on by default; turn it off to keep every
  literal URL as its own path.
- **Infer scalar parameter types** — type query/path params as `integer`, `number`,
  or `boolean` from their values; off makes every param a `string`.
- **Include captured examples** — attach a real captured value to each parameter and
  body schema; turn it off for a schema-only spec.
- **Host filter** — a case-insensitive substring to keep only requests to one host,
  handy for stripping analytics/CDN noise from a mixed capture.
- **API title** — set `info.title`; blank infers it from the first host.
- **Drop operations without a 2xx response** — remove error-only or never-completed
  calls from the output.

The result is a deterministic best-effort **draft** meant as a head start, not a
finished contract — review and edit it before publishing.

## FAQ

<details>
<summary>Does my HAR get uploaded anywhere?</summary>

No. The conversion runs as WebAssembly inside your browser tab — the HAR you paste
never leaves your machine, and there is no server, account, or network call involved.

</details>

<details>
<summary>Will this redact passwords, tokens, or cookies for me?</summary>

**No — a HAR routinely contains secrets** (auth headers, cookies, tokens, request
bodies), and this tool does not strip them. If your capture is sensitive, run it
through the **har-redact** tool first, then convert the cleaned HAR here.

</details>

<details>
<summary>How accurate is the generated spec?</summary>

It is a *draft* inferred purely from what the capture happened to show. A field only
appears if it was present in a captured body, an endpoint only appears if it was
called, and types are guessed from example values. Treat it as a scaffold to review
and refine, not as a validated, authoritative contract.

</details>

<details>
<summary>Why did two different URLs collapse into one path?</summary>

With **Parameterize id-like path segments** on (the default), segments that look like
ids — all-digit numbers, UUIDs, and long opaque tokens — are replaced with a
`{param}` template, so `/orders/1001` and `/orders/1002` merge into
`/orders/{order}`. Turn the option off to keep each literal URL as a separate path.

</details>

<details>
<summary>Does it detect authentication or security schemes?</summary>

No. Guessing `securitySchemes` from headers is noisy and easily wrong, so the tool
deliberately skips it. Add your API's auth definitions by hand after generating the
draft.

</details>

<details>
<summary>Can I get JSON instead of YAML, or a specific OpenAPI version?</summary>

Yes. Set **Output format** to JSON for `openapi.json`, and choose **OpenAPI version**
`3.0.3` or `3.1.0`. Both formats and versions describe the same inferred document.

</details>

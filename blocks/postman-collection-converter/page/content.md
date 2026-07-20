## Convert a whole collection at once

Exporting a collection from Postman (Collection v2.0 / v2.1 JSON) or Insomnia (JSON export,
format 4) gives you a single file with every request your team has built. This converter turns
that file straight into runnable code — one labeled snippet per request — instead of making you
open each request and copy snippets one by one:

- **cURL** — one command per request, with `-H`, `-u`, `--data-raw`, `--data-urlencode`, `-F`
  and friends, formatted across multiple lines with backslash continuations (or one line per
  command if you untick the option).
- **JavaScript fetch()** — modern `await fetch(...)` snippets with a headers object and a
  correctly typed body (`JSON.stringify(...)`, `URLSearchParams`, or `FormData`).
- **JavaScript axios** — `await axios({...})` snippets; JSON bodies become plain object
  literals and basic auth uses axios's native `auth` option.

Folders are flattened and each snippet is preceded by a comment with its folder path and request
name, so the output stays navigable. Everything runs locally in your browser — your collection,
tokens, and passwords are never uploaded.

## Worked example

Input (a Postman collection with one request):

```json
{
  "info": { "name": "Demo API" },
  "item": [{
    "name": "Create user",
    "request": {
      "method": "POST",
      "url": "https://api.example.com/users",
      "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "abc123" }] },
      "body": { "mode": "raw", "raw": "{\"name\":\"Ada\"}",
                "options": { "raw": { "language": "json" } } }
    }
  }]
}
```

Output with the cURL target:

```bash
# Create user
curl -X POST 'https://api.example.com/users' \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer abc123' \
  --data-raw '{"name":"Ada"}'
```

Switch the output format to fetch() and the same request becomes:

```js
// Create user
const response = await fetch('https://api.example.com/users', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': 'Bearer abc123',
  },
  body: JSON.stringify({"name":"Ada"}),
});
console.log(await response.text());
```

## What's covered

Per request: HTTP method (GET omits `-X`, HEAD becomes `curl -I`), the full URL (raw or
reassembled from Postman's structured `url` object), headers (disabled entries are skipped),
and every common body mode — raw text, JSON (a `Content-Type: application/json` header is added
when missing), `x-www-form-urlencoded`, multipart `form-data` (file fields become `@path` in
curl and placeholder `FormData` appends in JS), whole-file bodies, and GraphQL (wrapped into the
standard `{"query": ..., "variables": ...}` JSON body). Auth is honored at request level or
inherited from the collection: basic (curl `-u`, a base64 `Authorization` header for fetch,
axios's `auth` option), bearer tokens, and API keys sent as a header or query parameter.

`{{variable}}` placeholders are resolved from the collection's own variables and Insomnia
environments; the Variables field adds or overrides values (Postman environment export JSON, a
plain JSON object, or `KEY=VALUE` lines — both `{{name}}` and Insomnia's `{{ _.name }}` forms
are matched). Anything unresolved is left verbatim so you can spot it.

## Limits and edge cases

- At most **200 requests** per collection; larger collections are rejected with a clear count.
- Input must be **JSON**: Postman Collection v2.0/v2.1 or Insomnia export format 4. Postman v1
  collections and Insomnia's newer YAML exports aren't parsed — re-export as v2.1 / JSON.
- Pre-request and test **scripts are not executed**, and dynamic values such as `{{$guid}}` or
  `{{$timestamp}}` are not generated — they stay as-is in the output.
- File references (`@photo.png`, form-data file fields) keep the exported path; attach the real
  file when you run the snippet.

## FAQ

<details>
<summary>How do I export a collection from Postman or Insomnia?</summary>

In Postman, open the collection's ... menu and choose Export, keeping the recommended
Collection v2.1 format, then paste the saved JSON here. In Insomnia, use the collection's
Export option and pick the JSON format (export format 4). Both files are detected
automatically — you never have to say which app the export came from.

</details>

<details>
<summary>Are my API keys and passwords safe to paste?</summary>

Yes. The conversion runs entirely in your browser via WebAssembly — the collection, tokens,
and variable values never leave your machine. That said, generated snippets contain whatever
secrets the export contains, so treat the output with the same care as the export itself.

</details>

<details>
<summary>Why does my output still contain {{placeholders}}?</summary>

A placeholder is only replaced when a value for it is found — from the collection's own
variables, an Insomnia environment inside the export, or the Variables field. Anything without
a value is left verbatim so you can see exactly what still needs filling in. Add the missing
values as `KEY=VALUE` lines (or paste your Postman environment export) and convert again.

</details>

<details>
<summary>Which HTTP features are supported in the generated code?</summary>

Methods, URLs with query strings, headers, basic/bearer/API-key auth (request-level or
inherited from the collection), and raw, JSON, form-urlencoded, multipart form-data, file, and
GraphQL bodies. Disabled headers and fields are skipped, exactly as the app would skip them.
Pre-request scripts, test scripts, and cookie jars are outside the export's data and are not
reproduced.

</details>

<details>
<summary>Can it output Python, Go, PHP, or other languages?</summary>

Not currently — this tool focuses on curl commands and JavaScript (fetch and axios). For other
languages, the curl output is the most portable starting point: most HTTP client generators
accept a curl command as input.

</details>

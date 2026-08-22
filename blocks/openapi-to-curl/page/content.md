## About this tool

OpenAPI specs are great for machines, but during debugging you often just need a
pasteable `curl` command for every endpoint. This tool reads an OpenAPI 3.x or
Swagger 2.0 document, walks every path and operation, fills in sample path/query
/header/body values, and emits deterministic curl examples without sending any
network requests.

It prefers examples from the spec, then defaults, enum values and schema formats.
Local `$ref`s are resolved so shared schemas still produce useful bodies; remote
references are not fetched, so the tool stays offline and predictable.

### A worked example

Paste this tiny spec:

```yaml
openapi: 3.0.3
servers:
  - url: https://petstore.example.com/v1
paths:
  /pets/{petId}:
    get:
      summary: Get a pet
      parameters:
        - name: petId
          in: path
          required: true
          schema: { type: integer, example: 42 }
        - name: verbose
          in: query
          schema: { type: boolean, default: true }
      responses:
        '200': { description: OK }
```

With the default shell output, the generated command includes the server URL,
path parameter and query parameter:

```bash
BASE_URL="https://petstore.example.com/v1"

# GET /pets/{petId} — Get a pet
curl -X GET \
  "$BASE_URL/pets/42?verbose=true"
```

### Useful controls

- **Base URL override** swaps the server URL without editing the spec.
- **Authentication** can follow the spec or force bearer/basic/API-key placeholders.
- **Method, tag and path filters** narrow a large spec to the endpoints you care about.
- **Include optional params and fields** turns minimal examples into fuller examples.
- **Output format** switches between a shell script, bare commands, Markdown, and JSON records.
- **Schema expansion depth** prevents recursive or very large schemas from exploding.

### Limits and edge cases

- Remote `$ref` targets are not fetched; unresolved values collapse to `null`.
- The generated samples are examples, not contract tests. They may need real IDs,
  tokens or environment-specific hostnames before you run them.
- JSON and YAML OpenAPI 3.x and Swagger 2.0 documents are supported. A document
  without `paths` is rejected.
- Multipart and form request bodies are represented with curl `-F` / `--data-urlencode`
  style flags when the content type calls for it.

## FAQ

<details>
<summary>Does this make any HTTP requests?</summary>

No. It only parses the spec and writes example commands. The generated `curl`
commands are plain text; nothing is sent until you copy one into a terminal and
run it yourself.

</details>

<details>
<summary>Where do request body values come from?</summary>

The sampler uses the most specific hint available: explicit `example` values,
then `default`, then the first enum value, then a format-aware placeholder such as
an email, UUID or date-time. Object schemas include required fields by default;
turn on optional fields when you want a fuller sample body.

</details>

<details>
<summary>How are auth headers handled?</summary>

`auto` reads the operation or top-level security requirements and the declared
security schemes. If a credential value is blank, the output uses shell
placeholders such as `$TOKEN`, `$API_KEY` or `$API_USER:$API_PASSWORD` so secrets
are not baked into generated docs.

</details>

<details>
<summary>Can I generate examples for only part of a large API?</summary>

Yes. Use a comma-separated method filter (`get,post`), a tag filter (`pets`), or a
path substring (`/admin`). Filters combine, so a command must satisfy every
non-empty filter to be included.

</details>

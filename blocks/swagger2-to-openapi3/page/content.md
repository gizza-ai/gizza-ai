## About this tool

**Swagger 2.0** (also called **OpenAPI 2.0**) and **OpenAPI 3.0** describe the same API but
store it differently. Moving up to 3.0 unlocks newer tooling — code generators, mock servers,
documentation renderers, and gateways that only read the 3.0 shape. This converter does the
structural rewrite for you, in the browser: paste a Swagger 2.0 document as JSON or YAML and
get an equivalent OpenAPI 3.0 document back as JSON or YAML.

What it changes:

- **`swagger: "2.0"` → `openapi: "3.0.3"`** (pick any 3.0.x patch version).
- **`host` + `basePath` + `schemes` → a `servers` array** — one entry per scheme, or a
  protocol-relative / relative URL when a piece is missing.
- **`definitions` → `components.schemas`**, `securityDefinitions` → `components.securitySchemes`
  (`basic` → `http`/`scheme: basic`, and the oauth2 flow keys are renamed, e.g.
  `accessCode` → `authorizationCode`).
- **`in: body` and `in: formData` parameters → a `requestBody`**, with `consumes` folded into
  the content media types; a `type: file` form field becomes a `string`/`format: binary`.
- **Other parameters keep their place but move their type facets into a `schema`**, and
  `collectionFormat` becomes `style`/`explode`.
- **Response `schema` → `content`** keyed by the produced media types, and **every `$ref`** is
  retargeted from `#/definitions/…`, `#/parameters/…`, `#/responses/…` to the matching
  `#/components/…` path.

Everything runs locally via WebAssembly — your spec never leaves the page.

### Worked example

Input (Swagger 2.0):

```json
{
  "swagger": "2.0",
  "info": { "title": "Pet Store", "version": "1.0.0" },
  "host": "api.example.com",
  "basePath": "/v1",
  "schemes": ["https"],
  "paths": {
    "/pets": {
      "get": {
        "parameters": [
          { "name": "limit", "in": "query", "type": "integer", "format": "int32" }
        ],
        "responses": {
          "200": { "description": "A list of pets.", "schema": { "$ref": "#/definitions/Pet" } }
        }
      }
    }
  },
  "definitions": {
    "Pet": { "type": "object", "required": ["id"], "properties": { "id": { "type": "integer" } } }
  }
}
```

Output (OpenAPI 3.0.3):

```json
{
  "openapi": "3.0.3",
  "info": { "title": "Pet Store", "version": "1.0.0" },
  "servers": [ { "url": "https://api.example.com/v1" } ],
  "paths": {
    "/pets": {
      "get": {
        "parameters": [
          { "name": "limit", "in": "query", "schema": { "type": "integer", "format": "int32" } }
        ],
        "responses": {
          "200": {
            "description": "A list of pets.",
            "content": {
              "application/json": { "schema": { "$ref": "#/components/schemas/Pet" } }
            }
          }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "Pet": { "type": "object", "required": ["id"], "properties": { "id": { "type": "integer" } } }
    }
  }
}
```

### Options

- **Input format** — `auto` (default) tries JSON first, then YAML; force `json` or `yaml` to get
  format-specific error messages.
- **Output format** — `json` (default) or `yaml`.
- **Target OpenAPI version** — the exact `3.0.x` string stamped into `openapi`; `3.0.3` (the
  latest 3.0 patch) by default.
- **JSON indent** — spaces per level, `0`–`10`; `0` gives minified single-line JSON. Ignored for
  YAML output.
- **Patch missing 3.0-required fields** — on by default: fills the handful of fields 3.0 requires
  that 2.0 may omit (a missing `info.title`/`info.version`, missing response descriptions) so the
  result validates. Turn it off for a purely structural rewrite.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>What's the difference between Swagger 2.0, OpenAPI 2.0, and OpenAPI 3.0?</summary>

**Swagger 2.0** and **OpenAPI 2.0** are two names for the exact same specification — the format
was renamed "OpenAPI" after the project moved to the OpenAPI Initiative, so a file with
`swagger: "2.0"` at the top is an OpenAPI 2.0 document. **OpenAPI 3.0** is the next major
version, with a reorganized structure (`servers`, `components`, `requestBody`, `content`).
This tool reads the 2.0 form and writes the 3.0 form.

</details>

<details>
<summary>Does it convert to OpenAPI 3.1?</summary>

No — the output targets OpenAPI **3.0.x** (you can pick `3.0.0` through `3.0.3`). 3.1 is a
separate jump that realigns the schema dialect with full JSON Schema (for example `nullable`
is replaced by a `type` array and `example` by `examples`). Most 3.0-era tooling reads 3.0.x,
so that's the safe upgrade target; convert 3.0 → 3.1 afterward if you specifically need it.

</details>

<details>
<summary>Why did my body parameter turn into a requestBody?</summary>

OpenAPI 3.0 removed the `in: body` and `in: formData` parameter kinds. A single `in: body`
parameter becomes a `requestBody` whose `content` is keyed by the operation's `consumes` media
types (defaulting to `application/json`). Several `in: formData` fields are gathered into one
object schema under `application/x-www-form-urlencoded` — or `multipart/form-data` when a
`type: file` field is present, which also becomes `type: string`/`format: binary`.

</details>

<details>
<summary>What does the Patch option actually fill in?</summary>

3.0 makes a few things required that 2.0 tolerated as missing. With **Patch** on (the default),
an absent `info.title` becomes `"API"`, an absent `info.version` becomes `"1.0.0"`, and any
response with no `description` gets `"No description"` — the minimum needed for the result to
validate. It also lets you convert a document that omits the `swagger: "2.0"` marker entirely.
Turn Patch off if you want a strictly structural rewrite and would rather see those gaps.

</details>

<details>
<summary>Are there transforms it doesn't handle?</summary>

Yes — this is a deterministic converter for the common cases, not a full validator. It does not
resolve **external `$ref`s** (references into other files or URLs are left as-is), it drops a
shared **body/formData parameter** defined under top-level `parameters` (3.0 can't store those in
`components.parameters`), and it doesn't translate the rare `collectionFormat: tsv`. It also
doesn't validate the result — run it through an OpenAPI validator if you need a conformance
guarantee. Everything runs in your browser, so very large specs are bounded by the tab's memory.

</details>

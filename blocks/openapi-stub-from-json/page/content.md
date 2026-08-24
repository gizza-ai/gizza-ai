## What this tool does

Paste sample request and response JSON and this tool generates a complete OpenAPI 3.1 path-and-operation stub. It infers JSON Schema from the samples, wires those schemas into `requestBody` and `responses`, adds path and query parameters, and can emit YAML or JSON for pasting into a larger spec.

It is designed for the early documentation step: you have example payloads from a client, a test fixture, a log, or a prototype, and you want a deterministic OpenAPI starting point instead of an empty path block.

It can generate:

- request and response body schemas from JSON objects, arrays, scalars, and nulls;
- `required` lists based on observed object keys;
- string formats such as email, URI, UUID, date, date-time, and IPv4;
- component schemas plus `$ref`, or inline schemas;
- path parameters from `/things/{thingId}` and typed query parameters from a sample query string;
- optional bearer, basic, or `X-API-Key` security boilerplate;
- optional generic 400 and 500 error responses.

## Worked example

Use this request:

```json
{"name":"Ada Lovelace","email":"ada@example.com","active":true}
```

and this response:

```json
{"id":7,"name":"Ada Lovelace","email":"ada@example.com","created_at":"2026-08-21T07:00:00Z"}
```

with **Method: POST**, **Path: `/users`**, and **Operation ID: `createUser`**. The YAML output includes a path operation like:

```yaml
openapi: 3.1.0
paths:
  /users:
    post:
      operationId: createUser
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/CreateUserRequest'
```

The generated component schemas infer `email` as a formatted string, `active` as boolean, `id` as integer, and `created_at` as `date-time`. Review the descriptions, validation rules, enum choices, headers, and auth scopes before publishing the spec.

## Limits and edge cases

- This generates **one operation per run**. Use it repeatedly for multiple endpoints, or use a HAR-to-OpenAPI workflow when you have a whole captured session.
- A sample can only show what it contains. It cannot infer enums, minimum/maximum values, regex patterns, length limits, auth scopes, headers, pagination semantics, or business descriptions.
- Empty arrays become arrays with unconstrained item schemas; `null` values become OpenAPI 3.1 JSON Schema null types but do not reveal the intended non-null type.
- Array-of-object samples are merged across every element. Keys present in every object stay required; keys seen only in some elements become optional.
- The output is a stub. Treat it as a structured first draft, then edit names, summaries, descriptions, examples, response coverage, and validation constraints.

## FAQ

<details>
<summary>Is the output OpenAPI 3.0 or 3.1?</summary>

The document uses OpenAPI 3.1.0. That means JSON Schema nulls are represented with the JSON Schema type system instead of the older OpenAPI 3.0 `nullable` keyword.

</details>

<details>
<summary>Does it send my JSON anywhere?</summary>

No. The page runs the Rust WebAssembly generator in your browser, and CLI/chat runs are local to the gizza runtime. There is no network fetch or remote schema service.

</details>

<details>
<summary>How are path parameters generated?</summary>

Any braced path segment, such as `/users/{userId}`, becomes a required `in: path` parameter with a string schema. The sample JSON does not know the path parameter type, so tighten it by hand if the ID is numeric or UUID-shaped.

</details>

<details>
<summary>When should I turn off component schemas?</summary>

Leave components on when you want reusable request/response schemas. Turn them off for tiny examples, documentation snippets, or tests where an inline schema is easier to read.

</details>

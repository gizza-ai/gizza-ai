## What this tool does

Paste an OpenAPI 3.x or Swagger 2.0 document and this tool generates a single TypeScript file: a dependency-free client that calls your API with `fetch`. It focuses on the operations layer (`paths`): function names, path parameters, query parameters, header parameters, request bodies, response types, and the small runtime helpers needed to call `fetch` safely.

Use it when you already have, or plan to generate, schema types from `components.schemas` separately. Local `$ref` schema names become imported TypeScript types from your `types_module`; blank `types_module` emits placeholder aliases so the output still compiles while you wire real types in later.

It can generate:

- exported async functions or one client class;
- operation names from `operationId` or method + path;
- single request-object calls or positional path/body arguments;
- `ApiError` throwing or `{ data, error, response }` result unions;
- base URLs from the spec or from an override;
- typed path, query, header, body, and response references;
- JSDoc from summaries/descriptions and `@deprecated` markers.

## Worked example

Use a spec like this:

```json
{"openapi":"3.1.0","servers":[{"url":"https://api.example.com"}],"paths":{"/pets/{petId}":{"get":{"operationId":"getPet","parameters":[{"name":"petId","in":"path","required":true,"schema":{"type":"string"}},{"name":"include","in":"query","schema":{"type":"string"}}],"responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}}}}}}}
```

With the default **functions**, **request object**, and **throw ApiError** settings, the output contains a call shaped like:

```ts
export interface GetPetRequest {
  petId: string;
  include?: string;
}

export async function getPet(params: GetPetRequest, options: RequestOptions = {}): Promise<Pet> {
  return apiFetch("GET", `/pets/${encodeURIComponent(String(params.petId))}`, { include: params.include }, {}, undefined, options);
}
```

Point `types_module` at your generated schema declarations (for example `./types`) so local refs such as `#/components/schemas/Pet` are imported as `Pet`. Use `base_url` when you want the generated file to call staging, production, or a test server regardless of the spec's `servers` section.

## Limits and edge cases

- This generates one text file, not a multi-file SDK package. It does not create `models/`, `apis/`, npm metadata, or a runtime library folder.
- It reads `paths` operations only. It does not generate schema model declarations from `components.schemas`; use a schema-types generator for that and import those types with `types_module`.
- Only local refs such as `#/components/schemas/Pet` or `#/definitions/Pet` become named TypeScript types. External or remote refs fall back to `unknown` because this tool does not fetch the network.
- `application/json` request/response bodies are preferred; otherwise the first declared content type is used. Form-data and cookie parameters are noted in comments but are not automatically assembled.
- The success type comes from the lowest 2xx response (then `default`). Other response shapes are runtime `unknown` in the thrown `ApiError` or result union.
- Auth helpers, retries, middleware, interceptors, and date revivers are intentionally left to `RequestOptions.headers`, `RequestOptions.fetch`, or your surrounding app code.

## FAQ

<details>
<summary>Does this replace a full OpenAPI SDK generator?</summary>

No. It is intentionally smaller: one dependency-free TypeScript file for operations over `fetch`. Full SDK generators can produce package metadata, model files, API class folders, auth middleware, and many naming knobs; this tool is for a quick client layer you can paste into an existing project.

</details>

<details>
<summary>Where do the TypeScript model types come from?</summary>

Local schema refs become imported names from `types_module`, which defaults to `./types`. Generate those model types separately, or leave `types_module` blank to emit placeholder `type Name = unknown` aliases while prototyping.

</details>

<details>
<summary>Can it call private or remote refs while generating?</summary>

No. The block is pure Rust/WASM and never fetches referenced documents. External refs are represented as `unknown` so the generated client remains deterministic and safe to run locally.

</details>

<details>
<summary>How should I add authentication?</summary>

Pass auth headers with `RequestOptions.headers`, either per call or in the client-class constructor. For retries, refresh tokens, logging, or tracing, pass a wrapped `fetch` implementation through `RequestOptions.fetch`.

</details>

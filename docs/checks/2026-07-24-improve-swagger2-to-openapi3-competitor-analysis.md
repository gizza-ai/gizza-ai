# swagger2-to-openapi3 competitor analysis (2026-07-24)

## Scope

Tool: `swagger2-to-openapi3` — upgrade Swagger/OpenAPI 2.0 JSON or YAML documents into OpenAPI 3.0 JSON/YAML.

## Competitor scan

Sources reviewed from search results:

1. Swagger Converter (`swagger-api/swagger-converter`, converter.swagger.io)
   - Table stakes: accepts OpenAPI/Swagger 2.0; outputs OpenAPI 3.0; works as a hosted UI/API and Docker service; supports JSON/YAML specs.
   - UX patterns: paste/upload or API call; conversion result returned as a spec document.
   - In model: JSON/YAML input and output, 2.0-to-3.0 structural migration.
   - Out of model: hosted conversion API, Docker service, full official validator-grade edge coverage.

2. `swagger2openapi` CLI (referenced by DEV/Stainless docs)
   - Table stakes: command-line conversion; JSON/YAML input; YAML output flag; writes output file; handles common structural migration such as `definitions` to `components.schemas` and request bodies.
   - UX patterns: explicit output format flags and local/offline conversion.
   - In model: local/offline conversion, output format option, deterministic text output.
   - Out of model: full Node package option matrix, file writes from browser UI, resolving remote/external refs.

3. Swagger Editor conversion action (Stack Overflow reference)
   - Table stakes: paste/edit an OpenAPI 2.0 document and invoke a menu action to convert to OpenAPI 3.
   - UX patterns: interactive editor-style textarea, visible converted spec, validation handled separately by editor.
   - In model: paste spec, convert in-browser, show converted spec.
   - Out of model: full editor UI, schema validation annotations, menu-driven workflow.

## Decisions for this implementation

Built in-model table stakes:

- Accept JSON or YAML input (`input_format=auto|json|yaml`).
- Emit JSON or YAML (`output_format=json|yaml`).
- Select OpenAPI 3.0 patch target (`3.0.0`–`3.0.3`).
- JSON indentation control including minified output (`indent=0`).
- Optional patching of fields OpenAPI 3 requires but Swagger 2 may omit.
- Structural migrations for common specs: `swagger` to `openapi`, `host`/`basePath`/`schemes` to `servers`, `definitions` to `components.schemas`, `securityDefinitions` to `components.securitySchemes`, body/formData parameters to `requestBody`, non-body parameters to schema-wrapped parameters, response schemas to content, and internal `$ref` retargeting.

Explicit out-of-model / documented limits:

- No external `$ref` fetching or bundling.
- No full OpenAPI validation/linting.
- Rare conversion nuances such as every possible `collectionFormat`, discriminator migration nuance, and all vendor-tool option flags are outside this lightweight local converter.
- No hosted API/Docker service/editor UI; this repo ships the generic tool page and CLI surface only.

## Verification targets

- Unit tests cover JSON/YAML parsing, servers, components, requestBody/content, ref rewrites, target versions, patch=false, and errors.
- CLI checks should include JSON input exact content assertions, YAML output, target version, indent 0, and patch=false.
- Page tests should include exact output snippets, deep-link query params, enum/checkbox states, and malformed input error handling.

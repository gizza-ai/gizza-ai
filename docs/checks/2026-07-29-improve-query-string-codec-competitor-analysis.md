# query-string-codec — competitor analysis (2026-07-29)

Function: parse URL query strings into structured JSON and build encoded query strings from JSON,
including repeated keys, bracket notation, percent encoding, and multiple array serialization styles.
Pure-compute; runs entirely in browser / CLI / chat.

## Competitors surveyed

| # | Tool | Parse query → object | Build object → query | Arrays | Encoding controls | Notes |
|---|------|----------------------|----------------------|--------|-------------------|-------|
| 1 | qs / npm package docs | yes | yes | brackets, indices, repeat, comma-like | plus/form options | Developer library, not a no-install page |
| 2 | URLSearchParams playgrounds | yes | yes | repeated keys | browser encoding | Flat only, no bracket nesting |
| 3 | Code Beautify query string parser | yes | partial | repeated keys | minimal | Page-focused parser |
| 4 | FreeFormatter URL parser | yes | no | flat | no | URL inspection, not build mode |
| 5 | online JSON-to-query-string converters | no | yes | varies | minimal | Build direction only |

Paraphrased from public docs/tool pages; no competitor copy or branding reused.

## Table-stakes → decision

| Capability | Decision |
|------------|----------|
| Parse `a=1&b=2` into JSON | **IN** — default `direction=parse`. |
| Strip a leading `?` | **IN** — common copy/paste shape. |
| Percent-decode and `+`-decode | **IN** — `space_as_plus` controls form vs strict plus behavior. |
| Repeated keys → arrays | **IN** — `color=red&color=blue` becomes an array. |
| Bracket notation | **IN** — `tags[]`, `tags[0]`, and `user[age]` become nested JSON. |
| Build JSON → query | **IN** — distinct capability not covered by parse-query-string. |
| Array serialization styles | **IN** — `brackets`, `indices`, `repeat`, `comma`. |
| Sort keys | **IN** — deterministic output for docs/tests. |
| Leading `?` output | **IN** — `prefix_question_mark`. |
| Type inference on parse | **OUT** — query strings are text; parsed values stay strings. |
| Full URL parsing | **OUT** — sibling URL tools cover complete URLs; this tool is only the query component. |

## Relationship to existing blocks

`parse-query-string` covers the parse-only half. `query-string-codec` is not a strict duplicate because
it adds the reverse JSON→query-string build direction with array styles, sorted output, `%20` vs `+`,
and optional `?` prefix. The parse half is retained so users can round-trip and debug the codec in one
place.

## UX / page controls shipped

- `direction` select: query string → JSON or JSON → query string.
- `input` textarea for pasted query strings or JSON.
- `array_style` select with friendly labels.
- Checkboxes for plus spaces, sorted keys, and leading `?`.
- Example chips for parse, build brackets, and sorted `?` output.

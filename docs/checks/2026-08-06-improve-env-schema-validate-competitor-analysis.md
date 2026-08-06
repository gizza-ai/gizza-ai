# env-schema-validate — competitor analysis (2026-08-06)

Scan run while finishing the partial scaffold. Findings are paraphrased from how the
category behaves in general; no competitor copy, branding, trademarks or wording is reused.

## Function under study

Take a `.env` document plus a *declared* schema — which keys are required, what type each
value holds, which values are allowed — and report every variable that is missing, mistyped
or out of range, with the `.env` line number and a severity, before the app boots and fails
somewhere unrelated.

## Duplicate / viability check

Checked `blocks/` for env, dotenv, config and schema tools. The existing config/env tools
either parse or convert a config file, diff two environments, or lint formatting; none takes
a second *schema* input and validates key presence, value types and allowed values against
it. Pure Rust string/regex/JSON work, no I/O — a good fit for the local-wasm model.

## Competitors reviewed

### 1. Dotenv linters (dotenv-linter-style CLIs)

- What they actually check: file *hygiene* — duplicated keys, unordered keys, blank/leading
  characters, quote style, trailing whitespace, `.env` vs `.env.example` key drift.
- Table-stakes taken from them: duplicate-key detection with line numbers, understanding
  `export ` prefixes, quotes, inline `#` comments and blank lines.
- Structural gap: a linter only knows what is *in* the file. Nothing declares that `PORT`
  must exist and be a TCP port, so type/range/enum errors are invisible to it. Schema-shaped
  validation is the part the category leaves open.
- In-model decisions: keep the hygiene signals that matter (duplicates → warning, last-wins
  semantics stated), but make the schema the primary input.

### 2. Declarative dotenv schema files (dotenv-schema-style packages)

- Shape: a side-car schema file listing each variable with a type and a required flag, checked
  at load time by the library.
- Table-stakes: required vs optional, a small closed type vocabulary, defaults, per-key
  failure messages.
- Gap for a web tool: they run inside the app process at boot, so there is no way to check a
  file you were handed (a teammate's `.env`, a deploy-time paste) without wiring the library
  into a project first.
- In-model decisions: a one-line-per-variable rules dialect (`KEY=required|port`) so a schema
  can be typed straight into a text box, plus acceptance of a JSON Schema object for teams
  that already keep one.

### 3. Typed env validators (env-type-validator / envsafe / envalid-style)

- Shape: a schema declared in code (`port()`, `url()`, `email()`, `bool()`, enum/choices,
  min/max, defaults), validated at startup, exiting with a readable report of all failures
  at once rather than the first.
- Table-stakes taken: the type vocabulary itself (string, number, integer, boolean, port,
  url, email, host, JSON), enum/allowed values, numeric bounds, string length bounds, regex
  patterns, documented defaults, and reporting *every* problem in one pass.
- Notable UX behavior: they mask or omit secret values in their failure output, and they treat
  an empty string as unset by default because that is what dotenv loaders do.
- Gap: being code-first, they can neither be run against a pasted file nor produce a report
  that is safe to hand to someone else.
- In-model decisions: full type vocabulary, all-issues-at-once reporting, secret masking in
  every echoed value, `empty_is_missing` on by default with an explicit switch.

### 4. Online .env checkers / formatters

- Common controls: paste a file, get a formatted or de-duplicated result; a few compare
  `.env` against `.env.example`.
- Table-stakes UX: two multiline text areas, worked-example placeholders, preset examples,
  a copyable result.
- Trust gap: most are server-side, which is exactly wrong for a file full of credentials.
- In-model decisions: run entirely as local wasm, state that plainly on the page, mask
  secret-looking values so even the *result* is safe to paste into an issue or CI log.

## Gap list → decisions

| Capability | Fit | Decision |
| --- | --- | --- |
| Required / optional keys | in-model | Built: `required` / `optional`, bare `KEY=` line means required. |
| Value types (string, number, integer, boolean, port, url, email, host, json) | in-model | Built. |
| Allowed values | in-model | Built: `enum:a,b,c`, JSON Schema `enum` and `const`. |
| Numeric bounds vs string length | in-model | Built: `min`/`max` binds value for numeric types, character length otherwise. |
| Regex patterns | in-model | Built: `pattern:REGEX` (Rust `regex`, unanchored — documented on the page). |
| Secret strength policy | in-model | Built: `secure` (8+ chars, mixed case, digit, symbol). |
| Documented defaults | in-model | Built: `default:VALUE` downgrades a missing required key to a warning. |
| Existing `.env.example` as the schema | in-model | Built: `schema_format='example'`, and auto-detection of blank-valued template keys. |
| JSON Schema input | in-model | Built: `type`/`required`/`properties`, `enum`, `minimum`/`maximum`, `minLength`/`maxLength`, `pattern`, `format` and `default`. |
| Undeclared keys policy | in-model | Built: `unknown_keys` = warn / ignore / error. |
| Empty value = unset | in-model | Built: `empty_is_missing`, default true, switchable. |
| Duplicate keys, `export `, quotes, inline comments | in-model | Built: dotenv last-wins semantics, duplicates reported as warnings. |
| Machine-readable output for CI | in-model | Built: `output='json'` with `ok`, counts and an `issues[]` array carrying line/key/severity/rule/message. |
| Secret masking in output | in-model | Built: values of `SECRET`/`TOKEN`/`PASSWORD`/`KEY`/`AUTH`-ish keys are never echoed. |
| Reading `process.env` / the shell / files on disk | out-of-model | Not built — the page only sees the text you paste; stated in Limits. |
| `${VAR}` interpolation and multi-file layering (`.env.local` overrides) | out-of-model for this tool | Not built; values are validated literally, and the page says so. |
| Failing a build / git hook automatically | out-of-model | Not built; the JSON output plus its `ok` field is the integration point. |
| Generating typed accessors for an app | out-of-model | Not built; that belongs in the app's own loader. |

## Copy / UX notes taken into the page

- Lead with the failure being *moved earlier*: a bad env fails far from its cause at runtime.
- Make the local-only guarantee and the secret masking explicit, because the input is a file
  full of credentials — this is the category's biggest trust gap.
- Show a full worked example (schema, `.env`, exact report) so the rules dialect is learnable
  without reading a table first.
- Document the rules dialect as a table, and say plainly that an existing `.env.example`
  works as a schema unchanged.
- State the limits that surprise people: no `${VAR}` expansion, last-wins duplicates,
  `min`/`max` counts characters, unanchored patterns.
- Answer "how do I fail CI?" directly in the FAQ, pointing at the JSON `ok` field and the
  strict undeclared-key mode.

## Follow-up noted (not in this change)

Potential future additions are schema inference from a pasted `.env` and a documentation-table
output, but those are separate modes rather than table-stakes for validating a declared schema.

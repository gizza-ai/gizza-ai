# json-schema-compat-check — competitor analysis (2026-08-17)

Scan run BEFORE implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
Everything below is **paraphrased**; no competitor copy, branding, or trademarks are reproduced.

Backlog row: *"Compares an old and new JSON Schema to determine whether the change is
backward-compatible for producers and consumers."* (type hint: pure)

## Search

One WebSearch: "JSON Schema backward compatibility checker breaking change detection tool".
Real tools surfaced: chuckd, json-schema-diff-validator (npm + JVM rewrite), jsoncompat (Rust),
JSONSubschema (IBM research), a Zero Data Tools schema-evolution guide, and an in-progress
JSON-Schema-org GSoC compatibility-checker proposal.

Two candidate pages were unreachable and were **replaced** rather than run with fewer:
`npmjs.com/package/json-schema-diff-validator` returned 403 and `crates.io/crates/jsoncompat`
served no readable body. Substitutes: the Confluent Schema Registry schema-evolution reference
(the implementation chuckd wraps, i.e. the de-facto rule set) and the JVM
`lbenedetto/json-schema-diff-validator` rewrite of the npm package.

## Competitor 1 — chuckd (CLI over Confluent Schema Registry's checker)

Java/GraalVM CLI, meant for CI. Takes N previous schema files plus the new one (or a glob) and
reports whether the newest version is compatible with the earlier ones.

- **Compatibility direction is a first-class enum**: BACKWARD, FORWARD, FULL, and TRANSITIVE
  variants of each. Its own default is FORWARD_TRANSITIVE.
- Direction semantics it documents: *backward* = the new schema can read data produced under the
  earlier schema; *forward* = data produced under the new schema can be read by the earlier
  schema; *full* = both.
- **Output format enum**: TEXT (silent when compatible, details when not) or JSON (always valid
  JSON — empty array when compatible).
- Schema-format enum (JSON Schema / Avro / Protobuf), default JSON Schema.
- Distinct exit codes for compatible / incompatible / usage error / runtime error.
- Quiet flag to suppress metadata on stderr.

## Competitor 2 — Confluent Schema Registry JSON Schema compatibility rules

The reference rule set (chuckd and much of the ecosystem inherit it).

- Modes: BACKWARD, FORWARD, FULL, each with a TRANSITIVE variant, plus NONE.
- Producer/consumer framing rather than reader/writer: "a consumer using the new schema reading
  data written with the old schema" is the backward case.
- Two axes decide the verdict: a lenient-vs-strict policy, and whether the content model is
  **open** (`additionalProperties: true`) or **closed** (`additionalProperties: false`).
- Under strict + open: backward allows adding *and* removing optional fields and widening scalar
  types; forward allows removing optional fields and narrowing scalar types; full allows only
  adding/removing optional fields.
- Under strict + closed: backward allows adding optional fields and widening; forward allows
  removing optional fields and narrowing.

## Competitor 3 — json-schema-diff-validator (JVM rewrite of the npm package)

Library, not a CLI. Diffs old vs new schema and classifies each change.

- **Three severity levels per change category**: allowed → reported as info, discouraged →
  warning, forbidden → error. This is the key UX idea: a finding list with graded severity, not a
  single boolean.
- Per-category configuration, one knob each for: adding/removing `anyOf` branches, adding/removing
  enum values, adding/removing optional fields, adding/removing required fields, and adding or
  removing the `required` keyword itself.
- Result is a categorised list of issues (info / warnings / errors), not just a pass/fail.

## Table stakes → in-model / out-of-model

| # | Table stake (seen in ≥1 competitor) | Fit | Where it lands |
|---|---|---|---|
| 1 | Two schema inputs, old and new, pasted as text | in-model | `old_schema`, `new_schema` (required, multiline) |
| 2 | Compatibility **direction** selector with backward/forward/full | in-model | `direction` enum `full\|backward\|forward`, default `full` |
| 3 | Producer/consumer framing of each direction | in-model | every finding carries `breaks` = `old-producers` / `old-consumers`; page copy explains both |
| 4 | Open vs closed **content model** changes the verdict | in-model | `content_model` enum `auto\|open\|closed`, default `auto` (read from each schema's `additionalProperties`) |
| 5 | Graded severity per finding (error / warning / info) instead of one boolean | in-model | every finding has `severity` = `breaking\|warning\|compatible`; verdict = worst severity |
| 6 | Machine-readable **and** human-readable output | in-model | `output` enum `text\|json`, default `text` |
| 7 | Keyword-level rules: `required`, `type` widen/narrow, `enum` add/remove, numeric and length bounds, `pattern`, `additionalProperties`, `oneOf`/`anyOf` branch add/remove, `const`, `uniqueItems`, `multipleOf` | in-model | the core rule table (see `core/src/lib.rs`) |
| 8 | Annotation-only edits (title/description/examples/`$comment`/`default`) must not be reported as breaking | in-model | `ignore_annotations` boolean, default `true` |
| 9 | Ignore/allowlist specific paths | in-model | `ignore` (comma/newline-separated field names or dotted paths) |
| 10 | A way to fail CI on warnings too | in-model | `treat_warnings_as_breaking` boolean, default `false` |
| 11 | Local `$ref`/`$defs` resolution before comparing | in-model | pointer resolution with a cycle guard + depth cap; unresolvable refs become a `warning` finding |
| 12 | Bounded report size on large schemas | in-model | `max_findings` integer, default 200 (1–2000); input capped at 1 MiB per side |
| 13 | Preset examples for the common cases | in-model | four `[[example]]` chips (added required field, widened enum, tightened bound, safe optional add) |
| 14 | Distinct process exit codes per verdict | **out-of-model** | gizza blocks return a result document, not a process code; the verdict string is in the output for a caller to switch on |
| 15 | TRANSITIVE modes (check against *every* historical version) | **out-of-model** | needs an N-file schema registry/history; the block takes exactly two schemas. Two-at-a-time is the pairwise primitive a caller loops over |
| 16 | Avro and Protobuf schema formats | **out-of-model** | different schema languages; separate tools, not this row |
| 17 | Glob / directory input, filename-based version ordering | **out-of-model** | no filesystem in the browser sandbox; page and chat surfaces take pasted text |
| 18 | Remote `$ref` fetching (`https://…` refs) | **out-of-model** | pure block, no network; reported as an unresolved-ref warning rather than silently ignored |
| 19 | Full subschema-containment proof (JSONSubschema-style semantic subtyping) | **out-of-model** | a decision procedure over regex/numeric lattices; far beyond a pure keyword-rule engine. The page states this limit honestly: the tool is a keyword-level checker, not a formal subtype prover |
| 20 | Per-category configurable severity (10+ knobs, competitor 3) | **considered, rejected** | ten extra params for a page form is schema bloat; `treat_warnings_as_breaking` + `direction` + `ignore` cover the same intent with three knobs |

## Design decisions taken from the scan

- **Direction, not a bare boolean.** The row says "for producers and consumers", which is exactly
  the backward/forward split. Default `full` reports both sides at once, which is the useful
  default for a paste-two-schemas web tool (chuckd's CI-oriented default is transitive-forward,
  which has no meaning for a two-input tool).
- **Narrow vs widen is the single underlying axis.** A change that *narrows* the accepted value
  set breaks old producers (their still-valid data is now rejected) — the backward direction. A
  change that *widens* it breaks old consumers (they receive data their schema rejects) — the
  forward direction. Every rule in the core classifies a change as narrows / widens / both /
  neither, and the direction filter turns that into a severity.
- **Graded findings, worst-severity verdict** (from competitor 3) rather than chuckd's silent-on-
  success TEXT mode: the page always shows a verdict line plus counts so an empty result is never
  ambiguous.
- **`auto` content model** because a real pair of schemas usually declares
  `additionalProperties` itself; explicit `open`/`closed` exists for the schemas that don't.

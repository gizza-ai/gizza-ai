# mock-data-generator — competitor analysis (2026-06-22)

Tool: turn a compact shorthand schema (your own `field:type` pairs, with `[n]`
array repeats and `{ }` nested objects) into realistic mock JSON. Pure-Rust,
runs in the chat block, the CLI, and the standalone page; deterministic with a
non-zero seed.

## Differentiation vs the sibling `fake-data-generator`

`fake-data-generator` outputs **flat rows from a fixed column vocabulary**
(`full_name, email, phone, …` or `all`) as CSV/JSON/SQL/XML. It cannot express
**arbitrary field names**, **nested objects**, or **per-field arrays** — the
shape is always a flat table of canned columns. `mock-data-generator` is the
schema-shaped complement: the caller defines the JSON *structure* (names,
nesting, arrays, ranges, enums). They are distinct tools, not duplicates — one
is "fill these known columns", the other is "produce JSON in this shape I
describe". Confirmed by reading `blocks/fake-data-generator/src/lib.rs` (fixed
`fields` enum + flat output) before building.

## Top competitors surveyed

1. **Mockaroo** (mockaroo.com) — the market leader. Visual field builder, ~140
   data types, formulas, foreign-key/relational sets, REST mock endpoints, huge
   row counts behind sign-up, CSV/JSON/SQL/XML/Excel export.
2. **JSON Schema Faker** (json-schema-faker.js.org) — drives generation from a
   *standard JSON Schema* (+ Faker/Chance extensions). Strength: re-uses the
   schema you already have; weakness: verbose JSON Schema authoring.
3. **Faker.js / @faker-js/faker** — a code library, not a tool. Programmatic
   `faker.person.fullName()` etc.; the realistic-data vocabulary benchmark.
4. **json-generator.com** — a template DSL (`'{{repeat(5)}}'`, `'{{firstName()}}'`)
   producing JSON arrays. Closest in spirit to our shorthand.
5. **Mockend / Mockoon / generatedata.com** — schema-or-form → mock JSON/REST.
   generatedata.com is open-source, form-driven, many types, export formats.

## Capability diff and gaps

In-model gaps closed in this build:
- **Arbitrary field names + a type per field** — core feature, implemented.
- **Nested objects** (`user:{...}`) and **arrays of objects/values** (`field[n]`,
  `{...}[n]`) — implemented (json-generator/Mockaroo have these; the shorthand
  expresses both).
- **Numeric ranges + enums** (`int(lo..hi)`, `float(lo..hi)`, `enum(a|b|c)`) —
  implemented; these are table-stakes across all competitors.
- **Top-level array of N records** (`count`) — implemented, matching the
  "repeat" idiom every competitor exposes.
- **Reproducible seed** — implemented. Mockaroo/Faker support seeding; this is a
  differentiator vs the form tools that re-randomise every click.
- **Realistic-data vocabulary** — names, emails, usernames, phones, dates,
  addresses (street/city/state/zip/country), geo (lat/lng), network
  (url/domain/ipv4/mac), color, lorem (word/words/sentence/paragraph). Covers
  the common 80% of Mockaroo's catalogue.
- **Pretty / compact output** toggle — implemented.
- **Private + free + no sign-up** — runs entirely in-browser via WebAssembly;
  the schema and data never leave the device. This is the headline gizza
  advantage over Mockaroo/Mockend (server-side, account-gated, row-capped).

Out-of-model features deliberately NOT built (would need a server, a relational
engine, or capabilities outside gizza's single-block pure-Rust + single-input
model — listed, not implemented):
- **Relational / foreign-key consistency across datasets** (Mockaroo) — needs
  multi-entity state.
- **Live REST mock endpoints** (Mockaroo/Mockend/Mockoon) — needs a server.
- **CSV/SQL/XML/Excel export** — the page output surface renders text; JSON is
  the natural single output here. (The sibling `fake-data-generator` already
  covers CSV/SQL/XML for the flat-table case.)
- **Full standard JSON Schema input** (JSON Schema Faker) — intentionally traded
  for the far terser shorthand; a JSON-Schema front-end would be a separate tool.
- **Locale-specific data / massive 140-type catalogue** (Mockaroo/Faker) — the
  vocabulary here is English-locale synthetic; broad enough for fixtures.

## Copy / UX / SEO

- Title/description/tags target "mock data generator", "mock json", "test data
  generator", "api mocking", "json generator" — the high-intent queries, while
  staying distinct from the fake-data-generator page's "fake/dummy data" framing.
- Schema field is `multiline` so pasted multi-line schemas keep their newlines.
- `pretty` defaults to checked (indented JSON, the friendlier default).
- Content page documents the grammar, every supported type grouped by category,
  array/nesting syntax, the count + seed behaviour, and the privacy story.

NO competitor copy, branding, or trademarks were reproduced.

## Verification (all surfaces)

- `cargo test --workspace` — 14 core unit tests + 1 descriptor drift-guard pass.
- `wafer build` — chat `block.wasm` validates + instantiates (wasm32-wasip1).
- `wasm-pack build` — page wasm builds; `js-sys` clock supplies the seed=0 case.
- CLI — `gizza tool mock-data-generator …` verified for single object, nested +
  enum array (`count=2`), compact (`pretty=false`), and the invalid-type error
  (exit 1).
- Playwright — 4 page specs pass (JSON array, nested objects + arrays, compact
  single-line, invalid-type error message).

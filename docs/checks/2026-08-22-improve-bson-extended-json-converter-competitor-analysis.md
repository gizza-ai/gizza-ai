# bson-extended-json-converter — competitor analysis (2026-08-22)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool`. All notes are paraphrased
observations of behaviour; no competitor copy, branding, or trademarks are reproduced.

## Scope

Backlog row: "Converts MongoDB Extended JSON (with `$oid`, `$date`, `$numberLong`) to and from
plain JSON." Type hint `pure`.

## Duplicate check (cleared)

- `blocks/bson-inspector` takes **binary** BSON bytes (base64/hex) and renders a type tree or
  canonical Extended JSON. Its input is bytes; it never accepts Extended JSON *text* and never
  emits plain/unwrapped JSON. Disjoint surface — not a duplicate.
- `blocks/dynamodb-json-converter` is the same *shape* of tool (typed-wrapper JSON ↔ plain JSON)
  for a different wire format, and is the design precedent followed here.
- `blocks/mongo-query` / `blocks/jsonpath-query` query documents; they do not transcode types.

## Reference implementations examined

1. **MongoDB Extended JSON v2 specification** (mongodb/specifications) — the normative source for
   every wrapper form, canonical vs relaxed, and the legacy v1 shapes.
2. **Node driver `EJSON.parse` / `EJSON.stringify`** (bson package, documented in the Node driver
   manual) — the de-facto reference API.
3. **PyMongo `bson.json_util`** (`dumps`/`loads`, `JSONOptions`, `JSONMode`) — the other widely used
   reference implementation, and the one that exposes the most knobs.
4. **jsontotable.org "BSON to JSON"** — a representative free web tool, for UX/control expectations.
   (Substituted in for the generic "online converter" slot; its input is binary BSON, so only its
   UX patterns are load-bearing here.)

## Table stakes observed

| Capability | Seen in | Decision |
| --- | --- | --- |
| Unwrap Extended JSON → plain JSON | all three | **in-model** → `direction=to-plain` |
| Wrap plain JSON → Extended JSON | EJSON, json_util | **in-model** → `direction=to-extended` |
| Pick the direction automatically | (neither; ours) | **in-model** → `direction=auto` (default) |
| Canonical vs relaxed output mode | EJSON `relaxed:` flag, json_util `JSONMode` | **in-model** → `mode` |
| Default output is *relaxed* | EJSON (`relaxed:true`), json_util (RELAXED since 4.0) | **in-model** → `mode` defaults to `relaxed` |
| Accept legacy v1 input (`{"$date": 1699999999000}`, `{"$binary":"…","$type":"00"}`, `{"$regex":…,"$options":…}`) | json_util LEGACY, spec §legacy | **in-model** → parsed on input, always |
| Full type coverage, not just the three named in the backlog row | spec | **in-model** → 20 wrapper forms (see below) |
| Date rendering choice when unwrapping (ISO-8601 vs epoch millis) | json_util `datetime_representation` | **in-model** → `date_format` |
| Keep 64-bit / Decimal128 values exact rather than lossy JSON numbers | json_util `strict_number_long` | **in-model** → `big_numbers_as_strings` |
| Pretty vs compact output | every web tool | **in-model** → `pretty` (default on) |
| Preserve document key order | spec (BSON is ordered) | **in-model** → `serde_json` `preserve_order` |
| Top-level arrays as well as objects | EJSON, json_util | **in-model** → any JSON value is accepted |
| One-click sample / preset input | jsontotable.org "Sample" button | **in-model** → `[[example]]` preset chips |
| Copy + download the result | jsontotable.org | **platform** → generator ships both |
| Promote plain 24-hex / ISO-8601 strings to `$oid` / `$date` | none of the three | **in-model, opt-in** → `detect_types` (our differentiator; off by default because guessing types is lossy) |

### Type coverage committed to

`$oid`, `$date` (canonical, relaxed-ISO, and legacy integer), `$numberInt`, `$numberLong`,
`$numberDouble` (incl. `Infinity`/`-Infinity`/`NaN`), `$numberDecimal`, `$binary` (v2
`{base64,subType}` and legacy `{$binary,$type}`), `$regularExpression` and legacy
`{$regex,$options}`, `$timestamp`, `$minKey`, `$maxKey`, `$undefined`, `$symbol`, `$code`,
`$code`+`$scope`, `$dbPointer`. DBRef (`$ref`/`$id`/`$db`) is intentionally treated as an ordinary
subdocument, which is what it is in BSON — its `$id` is still transcoded recursively.

## Out of model (listed, not built)

- **Binary `.bson` file input / mongodump dumps.** Byte-level BSON is already covered by
  `blocks/bson-inspector`; adding a second decoder here would duplicate it.
- **Live MongoDB connections** (read a collection, write results back). No network in a pure block,
  and credentials do not belong in a URL-parameterised tool page.
- **UUID representation modes** (`uuid_representation`: legacy subtype 3 vs standard subtype 4
  reinterpretation). Requires driver-specific byte re-ordering rules that are not part of the
  Extended JSON spec; binary values pass through as base64 + subtype instead.
- **Driver code generation** (emit a PyMongo/Node snippet that reproduces the document).
- **Per-field type overrides via a UI type picker.** A grid editor is out of the page's
  single-run-per-input model.

## Non-obvious decisions

- **`auto` detection** keys off the presence of any *recognised* `$`-prefixed wrapper anywhere in
  the input: found → unwrap, otherwise → wrap. This means a plain document containing a literal
  `"$ref"` key is still treated as plain, since DBRef is not a wrapper.
- **Non-finite doubles always unwrap to strings** (`"Infinity"`, `"NaN"`) regardless of
  `big_numbers_as_strings` — JSON has no literal for them, so there is no lossless number choice.
- **Relaxed `$date` falls back to the canonical form outside years 1970–9999**, matching the spec
  rather than emitting an out-of-range ISO string.
- **`to-extended` re-parses Extended JSON input**, so `direction=to-extended` doubles as a
  canonical↔relaxed normaliser — a capability EJSON only gets via a parse-then-stringify round trip.

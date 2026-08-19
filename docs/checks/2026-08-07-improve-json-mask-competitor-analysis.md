# json-mask — competitor analysis (2026-08-07)

Scan run BEFORE implementation, per the build procedure. All notes are **paraphrased**
observations of publicly documented behaviour — no competitor copy, branding or code was
reproduced.

## Sources reviewed (top 3 real implementations of this grammar)

1. **nemtsov/json-mask** (JavaScript, the reference implementation of the language) —
   <https://github.com/nemtsov/json-mask>
2. **zapier/jsonmask** (Python port, "Google Partial Response dictionary pruning") —
   <https://github.com/zapier/jsonmask>
3. **teambition/json-mask-go** (Go port + HTTP middleware) —
   <https://pkg.go.dev/github.com/teambition/json-mask-go>

Context: the grammar originates in Google's Partial Response `?fields=` query parameter and
is the same syntax the GitHub-style "select a subtree" ask refers to.

## Table-stakes grammar (all three implementations agree)

| Form | Meaning |
| --- | --- |
| `a` | keep field `a` (and everything beneath it) |
| `a,b,c` | comma-separated sibling list |
| `a/b/c` | path — keep `c` inside `b` inside `a` |
| `a(b,c)` | sub-selection — from `a`, keep only `b` and `c` |
| `a/*/c` | `*` wildcard matches every key of an object / every element of an array |
| `\,` `\*` `\(` `\)` `\/` `\\` | backslash escapes a literal special character in a key name |
| `a,b/c(d,e(f,g/h)),i` | arbitrary nesting/combination of the above |

The reference grammar as documented:

```
Props ::= Prop | Prop "," Props
Prop  ::= Object | Array
Object::= NAME | NAME "/" Prop
Array ::= NAME "(" Props ")"
NAME  ::= visible chars except "\" | EscapeSeq | Wildcard
```

## Table-stakes semantics

- **Prune, don't extract.** The output keeps the *shape* of the input document — nested
  objects stay nested at the same paths. This is the key difference from JSONPath/jq/JSONata
  (which return a flat list of matched nodes) and is why this is a separate tool.
- **A bare name keeps the whole subtree.** `a` on `{"a":{"x":1,"y":2}}` keeps `a` intact;
  only a sub-selection (`a(x)`) prunes inside it.
- **Arrays map.** A sub-selection applied to an array is applied to every element, and the
  array (and its length/order) is preserved. `items(title)` over a 2-element array yields a
  2-element array of `{title}` objects.
- **Missing fields are silently omitted** by default in all three — masking a key that isn't
  present is not an error.
- **Invalid mask syntax is an error**, not a silent no-op (Go/Python both return an error;
  the JS parser throws).
- Scalars/`null` selected by a bare name pass through unchanged.

## Params / options each implementation exposes

| Implementation | Surface |
| --- | --- |
| nemtsov/json-mask (JS) | `mask(object, maskString)`; separate `compile()` + `filter()` for reuse; Express/Connect partial-response middleware bound to a `?fields=` query key |
| zapier/jsonmask (Py) | `parse_fields(mask)` → structure, `apply_json_mask(data, mask)`; Django REST helpers |
| teambition/json-mask-go | `Mask(doc []byte, fields string)`, `Compile(str)` → reusable `Selection`, `Selection.Mask(doc)`, plus `NewJSONMask(next, queryKey)` HTTP middleware |

**Observation:** none of the three exposes any *behavioural* option beyond the mask string
itself. Everything else is API ergonomics (pre-compilation for reuse, HTTP middleware) which
is irrelevant to a browser-local one-shot tool. So the grammar itself is the entire
table-stakes surface; option design is where a UI tool can add value.

Notable behavioural gap in the ports: the Go implementation emits object keys in sorted
order (Go map iteration + `encoding/json`), while the JS reference preserves source order.
Source-order preservation is the better default for a "show me a subtree of my document"
tool, so that is what we do (`serde_json` with `preserve_order`).

## UX choices worth copying

- Documentation always leads with a **worked example**: mask + input document + resulting
  output. The Google+ post example (`url,object(content,attachments/url)`) and the Go
  `kind,items(title,characteristics/length)` example both demonstrate paths, sub-selection
  and array mapping in one line. Our page copy should carry an equivalent original example.
- The syntax table above is presented as a compact cheat-sheet in every README. A tool page
  should render the same cheat-sheet rather than prose.
- All three frame the use case identically: shrink an API response to just the fields a
  client needs. That framing belongs in the page hero/FAQ.

## Decisions for our build

**Implemented (table stakes):** the full grammar — names, `,`, `/`, `(...)`, `*` wildcard
over both object keys and array elements, backslash escapes, arbitrary nesting; array
mapping; structure-preserving output; silent omission of absent fields by default; a hard
error with position/context for malformed masks.

**Implemented (added value, still in-model):**

- `mode = keep | remove` — `remove` inverts the mask (drop exactly what the mask selects,
  keep everything else). No competitor ships this; it is a natural inverse of "hiding the
  rest" and costs one branch in the same walker. Makes the tool double as a field-stripper.
- `format = pretty | compact` — a UI tool has to choose an output rendering; competitors are
  libraries returning objects, so this has no upstream analogue. Pretty (2-space) is the
  default because the output is read by a human here.
- `on_missing = omit | null | error` — `omit` reproduces competitor behaviour and is the
  default. `null` makes the output shape stable across records (useful when masking a list of
  records for a table/CSV). `error` turns a typo'd mask into a loud failure instead of a
  silently-empty result — the single most common complaint pattern with this syntax.
- `empty = keep | drop` — after masking, an object/array element that matched nothing is
  `{}`. Competitors keep it. `drop` removes those husks, which is what most people want when
  masking a big array. Default `keep` for compatibility.

**Considered, not built:** pre-compiled/reusable mask objects and HTTP middleware (API
ergonomics with no meaning for a one-shot browser tool); a `?fields=`-style server
integration (out of model — gizza is browser-local, no server).

# set-diff-json — competitor analysis (2026-08-18)

Scan done **before** implementation, to set the capability bar. All notes below are my own
paraphrase of publicly observable behaviour; **no competitor copy, wording, or branding was
copied** into the tool.

## Scope of the scan

There is no single dominant "JSON array set operations" web tool. The space splits into three
families, and the bar for this tool is the union of what they each do well:

1. **Generic set calculators** (miniwebtool set-theory calculator, onlinesettools union /
   intersection / difference pages, molbiotools list operations)
2. **JSON diff tools** (jsondiff.com, extendsclass JSON diff, jsonformatter compare,
   semanticdiff, diffchecker's JSON mode)
3. **Library primitives developers reach for instead** (lodash `unionBy` / `intersectionBy` /
   `differenceBy` / `xorBy`, jq set filters, ES2025 `Set.prototype.union` and friends)

## Family 1 — generic set calculators

**What they do well**

- All four operations in one place, named the way set theory names them: union, intersection,
  difference (A − B), symmetric difference. Miniwebtool also draws a Venn diagram and adds
  Cartesian product / power set.
- Counts are always visible: how big each input was, how big the result is.
- Set semantics by default — repeats inside one input collapse.
- Case-folding and whitespace-trimming toggles on the list-oriented ones.

**What they can't do**

- Items are **plain strings**, one per line or comma separated. Paste a JSON array of objects and
  they treat every character as text, so `{"id":1,"name":"Ada"}` and `{"name":"Ada","id":1}` are
  two different "items". There is no notion of matching records on an `id`.
- No JSON validation, no JSON output — you get a text list back, which you then have to
  re-quote by hand to feed anywhere else.

→ **Gap to close:** keep every operation, the counts, and the dedupe/case toggles, but make the
items *JSON values* compared semantically rather than textually.

## Family 2 — JSON diff tools

**What they do well**

- Real JSON parsing with clear syntax errors.
- Diffchecker's JSON mode is the strongest here: for arrays of objects it looks for a stable
  identity field (`id`, `name`, …) and pairs elements up by that field instead of by position,
  so a reordered or inserted record doesn't cascade into a wall of false differences.
- semanticdiff / extendsclass show a two-colour side-by-side "only on the left / only on the
  right" view, which is effectively A − B and B − A rendered visually.
- Several state plainly that the comparison happens in the browser.

**What they can't do**

- They answer *"how did this document change?"*, not *"what is the set relationship between
  these two collections?"*. There is no union, no intersection, no symmetric difference — and no
  way to ask for one specific side as a clean, re-usable JSON array.
- Output is a rendered diff view (colour spans, `path → old/new` rows), not a JSON array you can
  paste into the next step of a script.
- Identity-key matching is *inferred* rather than chosen: you cannot say "match on `sku`" when
  the records also carry an unrelated `id`.
- Reordering inside an object is usually treated as a non-change, which is correct — worth
  matching.

→ **Gap to close:** an explicitly chosen key field (not guessed), and a machine-usable JSON
result array rather than a diff view.

## Family 3 — library primitives

**What they do well**

- lodash `unionBy` / `intersectionBy` / `differenceBy` / `xorBy` are exactly the four operations,
  each with a "unique criterion" argument — a property name or an iteratee — which is the
  keyed-match idea in its cleanest form. `xorBy` is the symmetric difference.
- Convention worth inheriting: results are drawn from the **first** array, and the **first**
  occurrence of a duplicate key wins, so element order is predictable.
- jq and ES2025 `Set` methods give value-based union/intersection/difference for scalars.

**What they can't do**

- They're code, not a tool: you need a REPL, a `node -e`, or a jq install, and you must hand-roll
  the counts you actually wanted. jq's `-` / `unique` operate on whole values only, so keyed
  matching means writing a `group_by`/`INDEX` expression yourself.
- No validation story: a malformed paste is a stack trace.

→ **Gap to close:** be the no-install version of `differenceBy`, with the counts included.

## Table stakes → what ships in this tool

| Table stake | Seen in | Shipped |
| --- | --- | --- |
| union / intersection / difference / symmetric difference | families 1, 3 | ✅ `operation` |
| counts for both inputs and the result | family 1 | ✅ `counts` block in the output |
| set semantics (collapse repeats), toggleable | family 1 | ✅ `dedupe` (default on) |
| case-insensitive matching | family 1 | ✅ `case_insensitive` |
| real JSON parse + precise error | family 2 | ✅ line/column errors, per-array |
| object key order irrelevant when comparing | family 2 | ✅ canonical comparison key |
| match records on a chosen field | families 2 (inferred), 3 (explicit) | ✅ `key`, incl. dot-paths |
| first-array-wins, first-occurrence-wins ordering | family 3 | ✅ documented + tested |
| result usable as JSON downstream | family 3 | ✅ `output = array` mode |
| runs locally / nothing uploaded | families 1, 2 | ✅ WebAssembly, in-browser |

## Deliberately out of model

- **Venn diagram rendering** (miniwebtool) — the page renders a text/JSON result area; a diagram
  would need bespoke canvas work for little analytical gain over the counts block.
- **Side-by-side coloured diff view** (family 2) — that is `json-diff`'s job in this repo; this
  tool deliberately returns data, not a rendering.
- **Inferred identity keys** (diffchecker) — guessing which field is the identity is exactly the
  kind of silent wrong answer this tool should avoid. The key is explicit; leave it blank for
  whole-value comparison.
- **Arbitrary iteratee functions** (lodash) — no expression evaluation in a pure block. Dot-paths
  cover the property-name case that ~all real usage needs.
- **Cartesian product / power set** (miniwebtool) — not set *relationship* questions, and both
  explode in size on record-sized inputs.

## Neighbouring blocks (checked for overlap before building)

- `list-set-diff` — same set questions but over **plain text lines**; no JSON parsing, no keyed
  record matching, text report output. Complementary, not duplicative.
- `json-diff` — structural, path-by-path diff of two JSON *documents* (`$.list[2]` style paths).
  Answers "what changed", not "what is in A but not B".
- `list-dedupe-merge` — text union only.
- `sort-json-array`, `json-array-pluck-field` — single-array operations.

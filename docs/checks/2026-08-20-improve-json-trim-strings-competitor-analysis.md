# json-trim-strings — competitor analysis (2026-08-20)

Scan run BEFORE implementing, per `/create-next-tool` step 5 / `/improve-tool` Phase 2–3.
One `WebSearch` for the tool's function ("online tool trim whitespace from JSON string values",
plus a follow-up for recursive/key trimming), then the top real competitor tools were skimmed.
Everything below is **paraphrased** — no competitor copy, branding or trademarks reproduced.

## Search finding: the category is split, and nobody covers the middle

The search surfaced two clusters and essentially **no direct competitor** that trims string
*values* inside a parsed JSON document:

1. **JSON minifiers** — remove *structural* whitespace (between tokens), deliberately leaving the
   inside of every quoted string untouched. Opposite operation to ours.
2. **Plain-text trimmers** — trim lines of text with no idea what JSON is. Run one over a JSON
   document and it will happily eat the newlines and indentation *and* mangle the strings.

One sandbox page found in the search explicitly advertises the distinction as its selling point
(strip whitespace outside quotes only, escaped quotes accounted for) — i.e. the market treats
"whitespace inside JSON strings" as something to *protect*, not something you can clean. That is
exactly the gap this tool fills: the payload data is dirty, the structure is fine.

## Competitor 1 — a large online-tools suite's JSON minifier

- **Function:** compress JSON by dropping whitespace/newlines between tokens; structure preserved.
- **Params/options:** none. It is a single-button transform with no configuration at all.
- **Input:** paste, file import, or an `?input=` URL query parameter.
- **Output:** minified JSON text.
- **UX:** live preview as you type, copy-to-clipboard, download, paste-bin export, three worked
  example presets (a nested object, a social-media payload, a nested array), tool-chaining, and a
  documented URL-query API for automation.
- **Limits stated:** free tier is personal-use only, with daily caps and a download wait timer;
  the commercial licence and the wait-timer removal are paid.
- **Sidebar neighbours** relevant to whitespace: prettify, validate, syntax-highlight, edit/view.
  None of them touch string *values*.
- **Verdict:** in-model as a *formatting* concern only. Confirms that `indent` (pretty ↔ minify)
  is table stakes for any JSON-in/JSON-out tool, and that URL-query prefill + worked examples are
  table-stakes UX.

## Competitor 2 — the same suite's text trimmer

- **Function:** trim leading/trailing whitespace from text.
- **Params/options:** left-trim (on by default), right-trim (on by default), and a line-by-line
  mode that applies the trim per line rather than to the whole blob.
- **Input/output:** paste or import on the left, result renders instantly on the right.
- **UX:** import/save, copy, download, paste-bin export, tool chaining, `?input=`/`?left-trim=true`
  style query parameters, and three worked examples (both-ends trim, de-indenting paragraphs,
  trimming a line-based list).
- **Limits stated:** same free/paid split as above (personal use, daily quota, delays).
- **Verdict:** the **trim-side control is table stakes** — but expressed as two independent
  booleans. Decision below.

## Competitor 3 — a JSON-aware whitespace stripper (sandbox utility)

- **Function:** removes spaces, tabs and newlines that fall *outside* double-quoted strings, with
  escaped quotes handled correctly. In other words a lexer-level minifier.
- **Params/options:** essentially none; it is a single transform.
- **Notable positioning:** it calls out that many other tools also strip spaces *inside* values,
  which it treats as a bug.
- **Reachability:** the host refused connections at scan time (`ECONNREFUSED`); the behaviour above
  is from the search-result description, not a live page read. Flagged rather than claimed as
  verified.
- **Verdict:** reinforces that the two operations must never be confused. Our tool touches *only*
  string values (and optionally keys) and never the structure — and the page copy says so.

## Also checked / rejected as competitors

- A "remove extra spaces" page on a large code-beautifier site returned **HTTP 403** to the fetch,
  so it was not skimmed rather than guessed at. Its search snippet claims it accepts JSON as input,
  but as a plain-text pass (upload a file, spaces removed) — same class as competitor 2.
- Several results were tutorials/reference docs (`String.prototype.trim`, Jackson/Gson recipes,
  a Medium post on trimming JSON *keys*), not tools. The key-trimming article is a useful signal:
  people hit dirty **keys** as often as dirty values, which is why `keys` is a first-class param
  here rather than an afterthought.

## Table-stakes checklist → decisions

| Table stake (≥1 competitor) | Decision | Where |
| --- | --- | --- |
| Choose which end(s) to trim | **In model.** One `trim` enum (`both`/`leading`/`trailing`/`none`) instead of two booleans — the four-state enum makes "none" expressible and renders as a single `<select>` instead of an illegal both-off combination. Matches the sibling `csv-whitespace-normalizer`. | `trim` |
| Pretty vs minified output | **In model.** `indent` 0–8, default 2; `0` minifies. Covers the minifier competitor's whole feature as one field. | `indent` |
| Worked example presets | **In model.** Five `[[example]]` chips (default trim, collapse inner runs, trim keys, drop empties, minify). | `page/meta.toml` |
| URL-query prefill / automation API | **Already in model, for free.** Every gizza tool page pre-fills and auto-runs from `?param=value`, and the same descriptor drives the `gizza` CLI. | platform |
| Copy / download / reset | **Already in model.** The shared page runtime gives every text tool copy, download and reset. | platform |
| Live preview | **Considered, rejected.** `live = false`: on a large pasted document, re-parsing on every keystroke is a jank source. Run-on-submit is the family norm for parse-heavy JSON tools. | `page/meta.toml` |
| Free/paid gating, daily caps, wait timers | **Out of model — and the positioning angle.** Everything here runs locally in wasm with no account, no quota and no upload. Stated on the page as a capability, not as a jab at anyone. | copy |
| File upload / paste-bin export / tool chaining | **Out of model for this pass.** Paste + URL-param + CLI cover the same ground; the shared page runtime has no upload control for pure text tools, and adding one is a platform change, not a tool change. | — |

## Capability gaps we close that NO competitor covers

These come from the real failure mode (`" Berlin"` and `"Berlin"` are different join keys, and a
padded `" 42 "` breaks a downstream cast) rather than from any competitor's feature list:

- **Recursive** descent through objects and arrays to any depth — competitors are single-blob.
- **Interior** whitespace treatment (`internal` = `keep` / `collapse` / `remove`), so
  `"Ada   Lovelace"` → `"Ada Lovelace"` and `"AB 12 CD"` → `"AB12CD"`.
- **Unicode whitespace** (`whitespace` = `unicode` default / `ascii`): NBSP `U+00A0`, narrow NBSP
  `U+202F`, ideographic space `U+3000` — the characters that actually survive a spreadsheet or
  web copy-paste and are invisible in every editor.
- **Key trimming** (`keys`, default off) with a loud collision error instead of silent key loss.
- **Empty-after-trim policy** (`empty` = `keep` / `null` / `drop`), because `" "` → `""` is usually
  a missing value, not an empty string.
- **Scoped passes** (`only_keys` / `skip_keys`) so a password, hash or code-block field can be left
  byte-for-byte alone.

## Out-of-model, considered and not built

- Accounts, quotas, server-side batch processing, paid tiers — the whole model here is
  browser-local wasm.
- File upload / paste-bin export — needs a platform control this repo's shared pure-text page
  runtime doesn't have; would be a generator change, not a tool change.
- Zero-width character removal (`U+200B`, `U+FEFF`) — deliberately **not** whitespace here, matching
  the sibling CSV tool, and already covered by the existing `zero-width-cleaner` block. Called out
  in the page FAQ so users aren't left guessing.
- Type coercion of the trimmed result (`" 42 "` → `42`) — a different operation, already shipped as
  `json-coerce-types`. Cross-referenced in the FAQ instead of duplicated.

> Original work only — no competitor copy, branding or trademarks were reproduced.

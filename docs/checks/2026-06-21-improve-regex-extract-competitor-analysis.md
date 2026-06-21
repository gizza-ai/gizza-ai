# regex-extract — competitor analysis (2026-06-21)

Tool: `gizza-ai/regex-extract` — run a regular expression over text and return
every match, with case-insensitive / multiline / dot-all flags, capture-group
extraction, and optional deduplication. Pure-Rust (`regex` crate), runs on all
surfaces (chat, CLI, in-browser page).

## Surfaces verified

- **Chat/LLM API** — `descriptor()` single-sources the schema; drift-guard unit
  test `schema_json_matches_authored_chat_schema` passes.
- **CLI** — `gizza tool regex-extract text=… pattern=…` returns `{count, matches}`;
  capture-group and `unique` paths verified; invalid pattern returns a clear error.
- **Page** — `/tools/regex-extract/`; 3 Playwright tests pass (all-matches,
  capture-group, unique-dedupe).

## Top competitors surveyed

1. **regex101.com** — the reference online regex tester. Live highlighting,
   per-group breakdown, pattern explanation, debugger, multi-flavour (PCRE/JS/
   Python/Go/Rust), substitution, code-gen, quick-reference cheat sheet.
2. **regexr.com** — live tester with explanation pane, reference, and a
   community pattern library (JS flavour).
3. **regextester.com** — simple live tester focused on flags + capture groups.
4. **freetexttools.org "Extract by Regex"** — extraction-focused: pull every
   match (or a specific group) out of pasted text, with first/all modes and
   plain output. Closest in intent to this tool.
5. **rubular / pythex** — language-flavoured live testers (Ruby / Python) with
   group display and a quick reference.

## Gap analysis (fit-to-model)

Our tool's purpose is **extraction** (return every match), not a full
explain/debug IDE. Against the extraction-focused subset:

| Capability | Competitors | regex-extract | Status |
|---|---|---|---|
| Return all matches | yes | yes | covered |
| Extract a specific capture group | freetexttools, regex101 | yes (`capture_group`) | covered |
| Case-insensitive flag | all | yes (`ignore_case`) | covered |
| Multiline `^`/`$` flag | all | yes (`multiline`) | covered |
| Dot-matches-newline flag | all | yes (`dotall`) | covered |
| Deduplicate results | freetexttools (partial) | yes (`unique`) | **edge over most** |
| Match count | most | yes (`count`) | covered |
| Runs fully client-side / no upload | regexr/regex101 (client JS) | yes (wasm) | covered |
| Clear error on invalid pattern | all | yes | covered |
| Linear-time engine (no catastrophic backtracking) | Rust/RE2-class only | yes (`regex` crate) | **edge over PCRE tools** |

### In-model gaps closed this build

All of the above flags + capture-group + dedupe were implemented in the initial
build (not just the bare "match all"), so no follow-up gap-closing pass was
needed — the tool already matches or exceeds the extraction-focused competitors.

### Out-of-model features (intentionally NOT built)

These belong to full live-tester IDEs and are out of scope for a single
input→output gizza tool:

- **Live syntax highlighting / match overlay on the input** — needs a rich
  editor UI, not the page's text-in/text-out model.
- **Pattern explanation / regex debugger / step-through** — large feature, AST
  rendering; out of model.
- **Substitution / replace mode** — a different tool (replace, not extract); a
  separate `regex-replace` block would be the right home.
- **Multiple regex flavours (PCRE/JS/.NET)** — we expose one well-defined,
  linear-time flavour (Rust `regex`); emulating backtracking flavours is out of
  scope and would forfeit the no-catastrophic-backtracking guarantee.
- **Named-group output map** — could be a future enhancement; current model
  returns a single chosen group index, which covers the common extraction case.

No competitor copy, branding, or trademarks were reproduced.

## Sources

- [regex101: Capturing group](https://regex101.com/r/D5mUTo/1)
- [Free Extract by Regex — freetexttools.org](https://freetexttools.org/extract-text-with-regex/)
- [Capturing Groups and Back References — regextester.com](https://www.regextester.com/93594)
- [Groups and Backreferences — regular-expressions.info](https://www.regular-expressions.info/refcapture.html)

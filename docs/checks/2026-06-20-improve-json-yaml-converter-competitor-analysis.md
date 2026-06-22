# json-yaml-converter — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/json-yaml-converter` — convert between JSON and YAML in either
direction. Chat + CLI + page (pure-text field inputs; serde_json + serde_yml).

## What competitors do

- **Online JSON↔YAML sites** (json2yaml.com, onlineyamltools, transform.tools,
  cloudconvert) — paste, pick direction, copy result. Strengths: convenient.
  Weaknesses: most send the document to a **server** (config files often hold
  secrets), ads, and several are one-direction-only or mishandle types
  (everything becomes a string).
- **`yq` / `python -c`** — local + scriptable, but require installing tooling.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (serde_json + serde_yml/libyml)
   compiled to wasm: page runs in-browser, CLI headless, and it works in the chat
   Service Worker. Config data never leaves the device — important since YAML is
   usually config.
2. **Both directions + auto-detect.** `direction=auto` (default) infers from the
   input (`{`/`[` ⇒ JSON→YAML, else YAML→JSON); force with `json-to-yaml` /
   `yaml-to-json`.
3. **Type-preserving, round-trip safe.** Goes through `serde_json::Value`, so
   numbers/booleans/null/nesting survive; JSON→YAML→JSON returns the same data
   (verified by a round-trip test) — not the "everything-is-a-string" output some
   converters produce.
4. **Pretty or compact JSON** output (`pretty`, yaml-to-json).
5. **Clear errors.** Invalid JSON vs invalid YAML are reported distinctly.
6. **Three surfaces + deep-links.**

## Honest scope

- Emits standard block-style YAML (serde_yml). YAML anchors/aliases, comments,
  and custom tags are not round-tripped (comments are dropped — inherent to a
  data-model conversion).
- One document at a time (no multi-document `---` streams).

## Build note

`serde_yml` (libyml backend) compiles to wasm32-wasip1 cleanly and the block
instantiates in the wafer runtime; the page (wasm32-unknown-unknown) built
without needing the getrandom backend.

## Tests

6 core unit tests: JSON→YAML (mapping + sequence), YAML→JSON compact (types
preserved), YAML→JSON pretty (multi-line), a JSON→YAML→JSON **round-trip**
equality, auto-direction detection (4 cases), and error cases (empty, bad JSON,
bad YAML, bad direction). Plus the block drift-guard schema test. CLI + Playwright
(JSON→YAML via fill; YAML→JSON via deep-link) verified — see commit.

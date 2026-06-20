# improve-tool — reference (per-phase recipes, commands, templates)

Each `blocks/<slug>/` and `tools/generator` are SEPARATE cargo workspaces → `cd` into the
dir; do NOT use `-p <crate>` from repo root. `wafer build` runs from INSIDE `blocks/<slug>/`.

## Phase 1 — verify the three surfaces

The descriptor (`src/lib.rs` `descriptor()`) single-sources the chat schema, CLI, page, and
URL query-params. Verify all three live, from a known-good baseline:

- **API (LLM/chat schema + invoke):**
  - `cd blocks/<slug> && cargo test --workspace` (core + block units, incl. the drift-guard).
    The drift-guard lives in the *block* crate (`gizza-ai-<slug>-block`); a `tail`'d output can
    scroll it off — grep for the test name to confirm it actually ran.
  - `cd blocks/<slug> && wafer test` runs the `tests/*.json` fixtures (each a
    `{"kind":"invoke","data":[…bytes…],"meta":[]}` invoke payload). It is a no-crash/handles-input
    smoke gate, NOT an output assertion — assert exact outputs via the CLI + Playwright instead.
  - Confirm `descriptor()`/`info()` emits the tool schema (it's what the chat LLM sees).
- **CLI:** `cargo install --path cli --force`, then `gizza tool <slug> "<args>"` returns a
  correct result (same wasmi runtime + block + schema as chat).
- **Query params (page deep-link):** Playwright navigate `/tools/<slug>/?<param>=<value>` and
  assert the field pre-fills and the output computes. (The page reads descriptor params from
  the URL query string.)

If any fails: fix at root cause in `feat/improve-<slug>`, record under a "Phase-1 fixes" PR
section. If the breakage is far bigger than a focused fix → STOP and surface it.

## Phase 2 — competitor research (parallel)

**Search/scrape tools:** prefer `firecrawl-search` / `firecrawl-scrape` if they're connected;
otherwise fall back to the built-in **`WebSearch`** + **`WebFetch`** (verified working) — the
phase is tool-agnostic, just use whatever web research tools the session has.

1. Search the tool's function (e.g. "url encoder online", "image resize online") with
   `firecrawl-search` **or** `WebSearch`. Pick the **top 5** real competitor *tools* (a usable,
   reachable tool page — not a listicle or a login wall). If fewer than 5 real ones exist, say so
   and use what's real.
2. Dispatch **5 read-only subagents in parallel** (one per competitor URL). Subagent prompt:

   > You are researching ONE competitor tool for a gizza tool-improvement pass. Visit `<url>`
   > and return the competitor-profile JSON below. Use `firecrawl-scrape`/`firecrawl-extract`
   > (or `WebFetch` if firecrawl isn't available) for features/params, and Playwright for a
   > screenshot only if a picture beats markdown.
   > **PARAPHRASE everything — never copy their copy, branding, logos, or trademarks.** You are
   > a read-only researcher: do not edit any files.

   **competitor-profile schema** (each subagent returns this JSON):
   ```json
   {
     "name": "...", "url": "...",
     "features": ["..."],
     "params_options": [{"name":"...","type":"...","default":"...","range":"..."}],
     "input_formats": ["..."],
     "output_formats": ["..."],
     "output_quality": "...",
     "ux_patterns": ["presets / drag-drop / live-preview / grouping / ..."],
     "seo_copy_angles": ["topics & examples they rank for — PARAPHRASED, not verbatim"],
     "screenshots": ["optional/path.png"]
   }
   ```

## Phase 3 — diff + rank

Build a gap list: for each of the 4 dimensions, what ≥1 competitor does that our tool doesn't.
Tag each gap **in-model** / **out-of-model** (Global Constraints fit filter). Rank in-model by
value. Comprehensive run → all in-model gaps go to Phase 4. Out-of-model gaps → PR list only.

Dimensions: (1) **capabilities** (params/options/defaults/formats/output quality), (2) **copy +
SEO** (content.md / meta.toml), (3) **UX/layout** (page input-output presentation), (4) **visual
design** (page styling).

## Phase 4 — improve (per-dimension edit recipes)

- **Capabilities** — `core/src/lib.rs` (logic + a happy + an error `#[test]` per new behavior)
  and `src/lib.rs` `descriptor()` params. Param API:
  `Param::string|integer|number|enumv|boolean|string_map(...)` +
  `.required()/.default(v)/.min(n)/.max(n)/.describe(s)`; `Input::None` (pure) or
  `Input::Image|Video|Document|File` (media). `f64` for numerics. The descriptor single-sources
  the chat schema (`parameters = schema_json()` is pre-wired — do NOT hand-write inline JSON),
  the CLI, and query-params, so ONE edit updates all three. ffmpeg tools: also extend
  `web/src/lib.rs` `build_argv` + `page/meta.toml` field order (field order MUST equal the
  `build_argv` param order). Exemplars: `blocks/url-encode` (pure), `blocks/image-resize`
  (ffmpeg page), `blocks/web-fetch` (no-page).
- **Copy/SEO** — `page/content.md` (body, examples, FAQ) + `page/meta.toml`
  (title/description/tags/h1/hero). **Original copy only.**
- **UX/layout** — page input/output presentation; keep `[[input]]` field names + order in sync
  with the web export params.
- **Visual design** — page styling consistent with `gizza-chrome`. Original; no competitor assets.

### param types across surfaces (GOTCHA — bit me on url-encode)

A new param flows through three surfaces with DIFFERENT marshaling. Get this wrong and the page
silently passes a string where the wasm export wants a `bool`/`u32`:

- **Chat (LLM):** the descriptor type IS the wire type — `Param::boolean` → JSON `true`,
  `Param::integer` → JSON `2`. Deserialize into a typed serde `Args` field (`bool`, `u32`) with
  `#[serde(default)]`.
- **CLI:** `gizza tool` coerces `key=value` by the schema type (`integer`→i64, `number`→f64,
  `boolean`→true/false). So a typed descriptor "just works" on the CLI — no string handling needed.
- **Page (pure tools):** `tool.js` passes EVERY field's value as a raw STRING to the web `run`
  export (no coercion — only the ffmpeg path coerces numbers). So the web `run` signature must
  take `&str` for the new param and parse it there (e.g. `repeat: &str` → `parse::<u32>()`;
  `flag: &str` → match `"true"|"1"`). Do NOT make `run` take `bool`/`u32` — wasm-bindgen will get
  a string and throw. Keep the parse lenient (blank → default) and let the `core` fn own any
  clamp, so all three surfaces funnel through the same validation.
- A new page **bool/int field renders as a single-line text input.** If the param needs multi-line
  input (batch / per-line), set `multiline = true` on that `[[input]]` in `meta.toml` (renders a
  `<textarea>`); otherwise a pasted newline collapses to a space. `f64`/`u32` are fine as web
  `&str`-parsed; the wasm BigInt gotcha only applies if you (wrongly) type the export param as i64/u64.

### drift-guard — REGENERATE the authored schema

The block has a unit test pinning `derived == authored` (e.g. url-encode's
`schema_json_matches_authored_chat_schema`), with `authored` as a hardcoded JSON literal. An
`/improve-tool` schema change is INTENTIONAL, so:

1. PRINT the new derived schema (more reliable than transcribing the `assert_eq!` Debug diff,
   which shows `Value` debug, not JSON). Temporarily add `println!("DERIVED{}END", schema_json());`
   at the top of the drift-guard test, then:
   `cd blocks/<slug> && cargo test schema_json_matches -- --nocapture 2>&1 | grep -o 'DERIVED.*END' | sed 's/DERIVED//;s/END//' | python3 -m json.tool`.
   Remove the `println!` after. (Schema key order doesn't matter — `serde_json::Value` equality is
   order-insensitive; whole-number `min`/`max` render as JSON integers, e.g. `16` not `16.0`.)
2. **Replace the `authored` JSON literal** in `src/lib.rs`'s test with the new derived schema
   (add the new property/enum/default exactly as `to_schema_json()` emits it —
   `additionalProperties: false`, property order as inserted).
3. Re-run → PASS. Capture the **before→after schema diff** (old literal vs new) for the PR.
4. Update `manifest.json` `tool.description`/`tool.parameters` to match the new descriptor.

Do NOT delete the drift-guard test — it stays as the migration guard for the NEXT change.

## Phase 5 — re-test matrix (run from the stated dir)

- `cd blocks/<slug> && cargo test --workspace` — unit + drift-guard (regenerated) + core.
- wafer fixtures — the `tests/*.json` invokes (add one per new capability).
  Recipe: `python3 -c "import json;print(list(json.dumps({'<param>':'<v>'}).encode()))"` →
  the byte list goes in `{"kind":"invoke","data":[…],"meta":[]}`.
- `cd blocks/<slug> && wafer build` — wasm32 chat block (from INSIDE the dir; no path arg).
- `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` (from repo root).
- `cargo run --manifest-path tools/generator/Cargo.toml -- .` — renders `pkg/tools/<slug>/`.
  GOTCHA: the generator renders ALL tools (slug order) and HARD-ABORTS on the first whose
  `web/pkg/` is missing (a `wasm-pack`-not-yet-built tool) — so it may never reach `<slug>`. If it
  aborts on another tool, `wasm-pack build blocks/<that>/web …` for each missing one, then re-run.
- `solobase build` — rebuild app + blocks into `pkg/`. GOTCHA: it iterates+validates EVERY block
  and aborts on the first that fails — which may be a PRE-EXISTING, unrelated broken block (e.g. a
  `wasi_snapshot_preview1::sched_yield` validation error), not your tool. Confirm with
  `cd blocks/<that-block> && wafer build`; if it's unrelated and pre-existing, do NOT fix it here —
  validate YOUR tool with `cd blocks/<slug> && wafer build` and note the blocked full-app build in
  the PR (honesty gate). Do not claim `solobase build` passed if it didn't.
- **Playwright** `tests/tool-page-<slug>.spec.ts` (import from `./fixtures`) — drive
  `/tools/<slug>/`, AND a `?<param>=<value>` deep-link assertion. Add a case per new capability.
- **CLI** `cargo install --path cli --force` then `gizza tool <slug> "<args>"` — incl. a new
  case per new capability. (gpu tools: assert `unsupported_in_cli` + exit 3.)
- Hard gates: pre-existing behavior tests GREEN; every new capability has a test; API/CLI/query
  pass. ≤3 fix attempts per failure, else escalate.

### Known constraints (state in the PR when relevant)
- **chat ffmpeg is non-functional** — the chat runtime is a Service Worker where `import()`/
  `Worker` are forbidden; ffmpeg tools work via the standalone PAGE + CLI only.
- **gpu/imagine** — no headless GPU; build the chat block only, no page; CLI is
  `unsupported_in_cli`.

## Phase 6 — ship (review-only)

**PR body template:**
```md
## improve-tool: <slug>

### Competitor analysis (top 5)
| tool | does better | dimension |
| ---- | ----------- | --------- |
| <name> | <paraphrased> | capabilities/copy/ux/visual |
...

### Schema diff (before → after)
```diff
- <old authored schema literal>
+ <new authored schema literal>
```

### Changes by dimension
- **Capabilities:** <what + why>
- **Copy/SEO:** <what + why>
- **UX/layout:** <what + why>
- **Visual design:** <what + why>

### Phase-1 fixes (pre-existing breakage)
- <surface> was broken because <root cause>; fixed by <change>. (or: "none — all green")

### Out-of-model features considered, not built
- <feature> — needs <server/account/key>; doesn't fit browser-local/wasm.

### Tested
- unit + drift-guard (regenerated) · wafer fixtures · Playwright page+query · CLI · builds — results.
- Limitations: <e.g. chat-ffmpeg non-functional; gpu no headless page>.

> Original work only — no competitor copy, branding, or trademarks copied.
```

**Competitor-analysis snapshot:** commit `docs/checks/<YYYY-MM-DD>-improve-<slug>-competitor-analysis.md`
= the 5 paraphrased competitor-profiles + screenshots + the gap list (the archive record).

Then: `gh pr create` (review-only) → `/code-review` on the diff → post findings as a PR
comment → **do NOT merge**.

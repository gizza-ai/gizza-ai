# `/improve-tool` skill — design

**Date:** 2026-06-20
**Status:** Design approved; implementation plan next.
**Sibling:** `/new-tool` (`gizza-ai/.claude/skills/new-tool/`) — `/new-tool` *creates* a tool;
`/improve-tool` *upgrades an existing one*. Same structural shape, honesty gate, and build/test matrix.

## Goal

For one existing gizza tool, autonomously: (1) verify its three surfaces (LLM/chat API, CLI,
page query-params) actually work and fix any breakage at root cause; (2) research the top 5
competitor tools/websites for that function; (3) diff our tool against them; (4) close every gap
that fits gizza's model — across capabilities, page copy/SEO, page UX/layout, and visual design;
(5) re-run the full test matrix; (6) open a **review-only** PR with a competitor-analysis summary.
The PR is the only human gate (no mid-run pause); never merge.

## Locked decisions

- **Input:** `/improve-tool <slug>` (+ optional focus, e.g. "more formats"). **One tool per run.**
- **Autonomy:** fully autonomous → review-only PR. No mid-run approval. PR is the human gate. Mirrors `/new-tool`.
- **Improvement dimensions (all four in scope):**
  1. **Capabilities** — `descriptor()` params + `core` logic (missing options, weak defaults, missing
     formats, output quality). Highest value, lowest risk; single-sources chat/CLI/query-params.
  2. **Page copy + SEO** — `page/content.md` + `page/meta.toml` (copy, examples, FAQ, tags, title/desc).
  3. **Page UX/layout** — how the standalone page presents inputs/outputs (grouping, presets, preview).
  4. **Visual design/styling** — page styling, consistent with the shared `gizza-chrome` header/footer.
- **Hard rule — no copying:** analyze competitors for *ideas/features/UX patterns only*. Produce
  **original** copy, design, and assets. **Never** copy a competitor's text, branding, logos, or
  trademarks into our tool. The competitor-analysis artifact records *what they do*, paraphrased — not
  verbatim copyrighted text.
- **Scope per run:** **comprehensive** — close every gap that fits gizza's model in a single run/PR.
  Larger diff accepted; behavior tests must stay green and each new capability ships with a test.
- **Structure:** linear `SKILL.md` + `reference.md` (mirror `/new-tool`), **except Phase 2 fans out
  5 read-only research subagents in parallel** (one per competitor) so the editing phases run with a
  clean main-agent context.

## Fit-to-model filter (the improvement decision rule)

gizza tools are **browser-local, wasm, no-account, no-server**. A competitor feature is in-scope only
if it can run that way. Out-of-model examples → **listed in the PR as "considered, not built," never
forced in**:

- cloud/server batch processing, queues, async jobs
- logins / accounts / saved history / cloud storage
- API keys, paid tiers, usage metering
- anything requiring a backend the browser tool can't do locally

In-model examples → **build them**: more input/output formats, better defaults, extra transform params,
quality options, better copy/SEO, clearer UX, presets, nicer styling.

## Architecture

```
/improve-tool <slug>
        │
  ┌─────▼─────────────────────────────────────────────────────────┐
  │ Phase 1  VERIFY + FIX (known-good baseline)                    │
  │   API   : cargo test --workspace + wafer tests/*.json + info() │
  │   CLI   : gizza tool <slug> "<args>"                           │
  │   query : Playwright /tools/<slug>/?<param>=<value>            │
  │   → fix any breakage at root cause (same PR, called out)       │
  └─────┬─────────────────────────────────────────────────────────┘
        │
  ┌─────▼───────────────────────────────────────────────┐
  │ Phase 2  RESEARCH (parallel fan-out)                 │
  │   firecrawl-search → top 5 competitors               │
  │   5× read-only subagents (1/competitor) → profiles   │
  │   Playwright screenshots for visual/UX               │
  └─────┬───────────────────────────────────────────────┘
        │
  ┌─────▼───────────────────────────────────────────────┐
  │ Phase 3  DIFF + RANK                                  │
  │   our tool vs 5 profiles → gap list (4 dimensions)   │
  │   tag each gap fit-to-model (in / out-of-model)       │
  └─────┬───────────────────────────────────────────────┘
        │
  ┌─────▼───────────────────────────────────────────────┐
  │ Phase 4  IMPROVE (all fitting gaps)                  │
  │   capabilities: descriptor() params + core logic     │
  │   copy/SEO    : page/content.md + meta.toml          │
  │   UX/layout   : page input/output presentation       │
  │   visual      : page styling (gizza-chrome-aligned)   │
  │   drift-guard : REGENERATE expected schema           │
  └─────┬───────────────────────────────────────────────┘
        │
  ┌─────▼───────────────────────────────────────────────┐
  │ Phase 5  RE-TEST (did we improve it?)                │
  │   unit + wafer fixtures + Playwright(page+query) +   │
  │   CLI smoke + wafer build/wasm-pack/generator/       │
  │   solobase build. Behavior GREEN; new caps tested.   │
  └─────┬───────────────────────────────────────────────┘
        │
  ┌─────▼───────────────────────────────────────────────┐
  │ Phase 6  SHIP (review-only)                          │
  │   feat/improve-<slug> → PR (analysis + schema diff + │
  │   per-dimension changes + Phase-1 fixes + out-of-    │
  │   model list) + docs/checks snapshot + /code-review  │
  │   DO NOT MERGE                                        │
  └──────────────────────────────────────────────────────┘
```

## Phase detail

### Phase 1 — Verify + fix the three surfaces

The descriptor single-sources the chat schema, CLI, page, and URL query-params. "Works" is verified
on all three live surfaces, from a known-good baseline before any improvement:

- **API (LLM/chat schema + invocation):** `cd blocks/<slug> && cargo test --workspace`; run the wafer
  `tests/*.json` fixtures (block invoke); confirm `info()`/`descriptor()` emits the schema. This is the
  tool's callable contract for the chat runtime.
- **CLI:** `cargo install --path cli --force`, then `gizza tool <slug> "<args>"` returns a correct
  result (the CLI runs the SAME wasmi runtime + block + schema as chat — single source of truth).
- **Query params (page deep-linking):** Playwright navigates `/tools/<slug>/?<param>=<value>` and
  asserts the matching field pre-fills and the output computes.

Any failure here is **fixed at root cause** in the same PR and **called out as a separate "Phase-1
fixes" section** in the PR body. If Phase 1 uncovers breakage too large for a focused fix, **STOP and
surface it** rather than silently bundling a large rewrite (honesty gate).

### Phase 2 — Competitor research (parallel)

1. `firecrawl-search` for the tool's function → pick the **top 5** real competitor tools (web tools
   that do the same job; trust threshold: a usable, reachable tool page, not a listicle).
2. Dispatch **5 read-only subagents in parallel** (one per competitor). Each returns a **competitor
   profile** (schema below) using `firecrawl-scrape`/`firecrawl-extract` for features and Playwright
   for visual/UX screenshots where a picture beats markdown.
3. **No-copy rule enforced in the subagent prompt:** capture *what the competitor does* (features,
   params, defaults, formats, UX patterns, SEO angles) **paraphrased** — never copy verbatim copy,
   branding, logos, or trademarks.

**Competitor profile schema** (each subagent returns this):
```
{
  name, url,
  features:        [string],        // capabilities they offer
  params_options:  [{name, type, default, range}],
  input_formats:   [string],
  output_formats:  [string],
  output_quality:  string,          // notes on quality/options
  ux_patterns:     [string],        // presets, drag-drop, live preview, grouping…
  seo_copy_angles: [string],        // topics/examples they rank for (paraphrased, NOT verbatim)
  screenshots:     [path]           // optional Playwright captures
}
```

### Phase 3 — Diff + rank

Synthesize the 5 profiles against our tool (its `descriptor()` params + `core` behavior + page) into a
**gap list** across the four dimensions. Tag each gap **in-model** or **out-of-model** (fit filter
above). In-model gaps are ranked by value; all in-model gaps are slated for Phase 4 (comprehensive).

### Phase 4 — Improve (comprehensive)

Edit, per dimension:
- **Capabilities:** `core/src/lib.rs` (logic) + `src/lib.rs` `descriptor()` params (`Param::…` +
  `.required()/.default()/.min()/.max()/.describe()`). One edit propagates to chat schema, CLI, and
  query-params. For ffmpeg tools also update `web/src/lib.rs` `build_argv` + `page/meta.toml` field
  order. Use `f64` (never `i64`) for numeric params (wasm BigInt gotcha).
- **Copy/SEO:** `page/content.md` + `page/meta.toml` (title/description/tags/h1/hero + body). **Original
  copy only.**
- **UX/layout:** the page's input/output presentation (field grouping, presets, preview) — follow
  existing page patterns; keep field names/order in sync with the web export params.
- **Visual design:** page styling consistent with the shared `gizza-chrome` header/footer. **Original
  design; no competitor assets.**

**Drift-guard:** the per-tool drift-guard test asserts `derived == authored` schema. Because
`/improve-tool` **intentionally changes** the schema, the skill **regenerates the expected/authored
schema** to the new shape (it does NOT assert against the old one) and records the **before→after schema
diff** in the PR. Keep `manifest.json` `tool.description`/`tool.parameters` consistent with the new
`descriptor()` for hygiene + build.rs.

### Phase 5 — Re-test (the "did we improve it" gate)

Re-run the full `/new-tool` matrix against the edited tool:
- `cargo test --workspace` (unit + drift-guard with the regenerated schema)
- wafer `tests/*.json` fixtures
- Playwright page spec **including the query-param deep-link**
- CLI smoke (`gizza tool <slug> …`)
- `wafer build`, `wasm-pack build …/web`, generator, `solobase build`

**Hard gates:** every pre-existing behavior test stays GREEN; **each new capability ships with its own
test** (unit + a page/CLI assertion exercising the new option); all three surfaces (API/CLI/query) pass
post-edit. ≤3 fix attempts per failure, then escalate (honesty gate).

### Phase 6 — Ship (review-only)

- Branch `feat/improve-<slug>`, commit, push, `gh pr create`.
- **PR body:** competitor-analysis summary (the 5, what each does better) · before→after schema diff ·
  per-dimension change list (capabilities / copy-SEO / UX / visual) · **Phase-1 fixes** (separate
  section) · **out-of-model features considered, not built** · what was tested + results · any
  limitation (e.g. chat-ffmpeg non-functional; gpu has no headless page verification).
- Commit a `docs/checks/2026-06-20-improve-<slug>-competitor-analysis.md` snapshot (the paraphrased
  research record + screenshots) for the archive.
- Run `/code-review` on the diff; post findings as a PR comment.
- **Do NOT merge.**

## Error handling / honesty gate

Same as `/new-tool`:
- If a build/test fails unrecoverably (≤3 attempts), **STOP and report** with the error — never open a
  "done" PR for a broken tool.
- If competitor research is thin (fewer than 5 real competitors), say so and proceed with what's real.
- If a surface can't be headlessly verified (gpu/imagine has no page; chat-ffmpeg can't run in a Service
  Worker), state it explicitly rather than claiming a pass.
- Never claim a step passed that wasn't run.

## Components

- **`SKILL.md`** — the 6-phase autonomous procedure, the honesty gate, and the **no-copy/branding rule**,
  prominently. Front-loads: input → branch → Phase 1…6. Points at `reference.md` for the exact recipes.
- **`reference.md`** —
  - the verify-the-three-surfaces commands (Phase 1);
  - the **parallel-research subagent prompt template** + the competitor-profile schema + the no-copy
    instruction (Phase 2);
  - the per-dimension edit recipes (capabilities/copy-SEO/UX/visual) referencing the descriptor API and
    the exemplars (`blocks/url-encode`, `blocks/image-resize`, `blocks/web-fetch`) (Phase 4);
  - the **drift-guard-regenerate** recipe (Phase 4);
  - the full build/test command matrix (reused from `/new-tool`'s `reference.md`) (Phase 5);
  - the PR-body template + competitor-analysis snapshot template (Phase 6).

## Out of scope (YAGNI)

- Multi-tool sweeps (one tool per run; re-invoke for more).
- A separate research/apply skill split (single continuous flow).
- Auto-merge (always review-only).
- Building out-of-model (server/account/paid) competitor features.

## References

- Sibling: `gizza-ai/.claude/skills/new-tool/{SKILL.md,reference.md}`.
- Tool shape: `docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md`;
  exemplars `blocks/url-encode` (pure), `blocks/image-resize` (ffmpeg page), `blocks/web-fetch` (no-page).
- Handoff that seeded this: `docs/superpowers/handoffs/2026-06-20-improve-tool-skill-handoff.md`.
- Toolchain notes (pre-descriptor; SKILL.md/reference.md are current truth):
  `/workspace/docs/checks/2026-06-18-gizza-new-tool-build-notes.md`.

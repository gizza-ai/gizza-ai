# `/improve-tool` Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the autonomous `/improve-tool` gizza skill — given an existing tool slug, it verifies+fixes the tool's three surfaces, researches the top 5 competitors in parallel, closes every in-model gap across capabilities/copy-SEO/UX/visual, re-tests, and opens a review-only PR.

**Architecture:** Two prose files (`SKILL.md` + `reference.md`) under `gizza-ai/.claude/skills/improve-tool/`, sibling to `new-tool/`. The skill is validated the same way `/new-tool` was — a fresh zero-context subagent executes it end-to-end on a real tool and must produce a correct review-only PR. The "tests" for a prose skill are (a) structural checks on the files and (b) the proof-run with explicit pass criteria.

**Tech Stack:** Markdown skill files; gizza tool toolchain (`wafer build`, `wasm-pack`, `tools/generator`, `solobase build`, `gizza tool` CLI); firecrawl skills + Playwright MCP for research; the descriptor abstraction (`gizza_ai_block_utils::{ToolDescriptor, Param, Input}`).

## Global Constraints

- **Skill location:** `gizza-ai/.claude/skills/improve-tool/{SKILL.md,reference.md}` — sibling to `new-tool/`. Mirror `new-tool`'s shape and honesty gate.
- **Autonomy:** fully autonomous, no mid-run pause; the review-only PR is the only human gate; **never merge**.
- **One tool per run.** Input: `/improve-tool <slug>` (+ optional focus).
- **No-copy/branding rule (HARD):** analyze competitors for ideas only; produce **original** copy/design/assets; **never** copy a competitor's text, branding, logos, or trademarks. Research records are paraphrased, never verbatim.
- **Fit-to-model filter:** gizza tools are browser-local, wasm, no-account, no-server. Build only in-model gaps; list out-of-model features in the PR as "considered, not built."
- **Drift-guard regenerates:** the descriptor schema changes on purpose, so the skill **regenerates** the `authored` schema literal in the drift-guard test (it does NOT assert the old one) and records the before→after schema diff in the PR.
- **Hard test gates per run:** every pre-existing behavior test stays green; every new capability ships with its own test; all three surfaces (API/CLI/query) pass post-edit.
- **Numeric params:** `f64`, never `i64` (wasm BigInt gotcha).
- **PRs not direct-to-main.** The skill's own files land via branch `feat/improve-tool-skill`; each proof/real run opens its own `feat/improve-<slug>` PR.

**Spec:** `docs/superpowers/specs/2026-06-20-improve-tool-skill-design.md`.

---

### Task 1: Author `SKILL.md`

**Files:**
- Create: `gizza-ai/.claude/skills/improve-tool/SKILL.md`

**Interfaces:**
- Produces: the skill entrypoint — frontmatter `name: improve-tool` + a `description` that triggers on "improve an existing gizza tool"; the 6-phase procedure; the honesty gate; the no-copy rule. Phase steps reference `reference.md` (Task 2) for exact recipes.

- [ ] **Step 1: Write the failing structural check**

Create `gizza-ai/.claude/skills/improve-tool/check-skill.sh` (temporary, deleted in Step 5):

```bash
#!/usr/bin/env bash
# Structural gate for SKILL.md — every required element must be present.
set -euo pipefail
F=.claude/skills/improve-tool/SKILL.md
fail=0
need() { grep -q "$1" "$F" || { echo "MISSING: $1"; fail=1; }; }
need "^name: improve-tool$"
need "Phase 1"; need "Phase 2"; need "Phase 3"
need "Phase 4"; need "Phase 5"; need "Phase 6"
need "reference.md"
need "Honesty gate"
need "NEVER copy"          # no-copy/branding rule
need "review-only"; need "Do NOT merge"
need "fit-to-model"
[ "$fail" = 0 ] && echo "SKILL.md structural check: PASS" || { echo "SKILL.md structural check: FAIL"; exit 1; }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd gizza-ai && bash .claude/skills/improve-tool/check-skill.sh`
Expected: FAIL (`SKILL.md` does not exist yet → grep error / MISSING lines).

- [ ] **Step 3: Write `SKILL.md`**

Create `gizza-ai/.claude/skills/improve-tool/SKILL.md` with EXACTLY this content:

````markdown
---
name: improve-tool
description: "Use when the user wants to improve/upgrade an existing gizza tool. Takes a tool slug, verifies its three surfaces (chat/LLM API, CLI, page query-params) and fixes any breakage, researches the top 5 competitor tools in parallel, closes every in-model capability/copy/UX/visual gap, re-runs the full test matrix, and opens a review-only PR with a competitor-analysis summary. Fully autonomous; never copies competitor copy or branding; never merges."
---

# improve-tool — upgrade an existing gizza tool end to end

Autonomous: from an existing tool **slug** (+ optional focus), verify it works, study the
top 5 competitors, and close every gap that fits gizza's model. No mid-run confirmation —
the **review-only PR is the only human gate**; **never merge**. NEVER claim a step passed
that you didn't run. Sibling to `/new-tool` (which *creates* tools); this *upgrades* one.

Read `reference.md` (next to this file) for the exact commands, the parallel-research
subagent prompt + competitor-profile schema, the per-dimension edit recipes, the
drift-guard-regenerate recipe, the build/test matrix, and the PR templates.

**NEVER copy** a competitor's copy, branding, logos, or trademarks. Analyze them for
*ideas/features/UX patterns only*; produce **original** copy, design, and assets. The
competitor-analysis record is paraphrased, never verbatim.

**Fit-to-model filter:** gizza tools are browser-local, wasm, no-account, no-server. Build
only gaps that can run that way. Out-of-model features (cloud/server batch, logins/accounts,
API keys, paid tiers, anything needing a backend) are **listed in the PR as "considered, not
built," never forced in**.

Follow these phases in order:

1. **Gather + branch.** Take the tool `slug` (+ optional focus). If no slug is supplied,
   ask for it (the ONLY question allowed). Confirm `blocks/<slug>/` exists (`gizza list`).
   `git checkout -b feat/improve-<slug>` from `main`.

2. **Phase 1 — Verify the three surfaces, fix any breakage (known-good baseline).** The
   descriptor single-sources chat/CLI/page/query-params; verify all three live. See
   reference.md §"Phase 1":
   - **API** (LLM/chat schema + invoke): `cd blocks/<slug> && cargo test --workspace`; run
     the wafer `tests/*.json` fixtures; confirm `descriptor()`/`info()` emits the schema.
   - **CLI:** `cargo install --path cli --force` then `gizza tool <slug> "<args>"`.
   - **Query params:** Playwright `/tools/<slug>/?<param>=<value>` → field pre-fills + output
     computes.
   Fix any failure **at root cause**, in this branch, recorded as a separate "Phase-1 fixes"
   PR section. If the breakage is far bigger than a focused fix, **STOP and surface it**
   (Honesty gate) rather than bundling a large rewrite.

3. **Phase 2 — Competitor research (parallel fan-out).** `firecrawl-search` the tool's
   function → pick the top 5 real competitor tools. Dispatch **5 read-only subagents in
   parallel** (one per competitor); each returns the competitor-profile schema in
   reference.md §"Phase 2" (features, params/options, defaults, input/output formats, output
   quality, UX patterns, SEO angles — **paraphrased**; + optional Playwright screenshots).
   Enforce the no-copy rule in each subagent prompt.

4. **Phase 3 — Diff + rank.** Synthesize the 5 profiles vs our tool (`descriptor()` params +
   `core` behavior + page) into a gap list across the 4 dimensions. Tag each gap **in-model**
   or **out-of-model**. All in-model gaps are slated for Phase 4 (comprehensive).

5. **Phase 4 — Improve (every in-model gap).** Edit per dimension (reference.md §"Phase 4"):
   - **Capabilities:** `core/src/lib.rs` logic + `src/lib.rs` `descriptor()` params (new
     options, better defaults, more formats, better output). `f64` not `i64` for numerics.
     For ffmpeg tools also update `web/src/lib.rs` `build_argv` + `page/meta.toml` field order.
   - **Copy/SEO:** `page/content.md` + `page/meta.toml` — original copy, examples, FAQ, tags,
     title/description.
   - **UX/layout:** the page's input/output presentation (grouping, presets, preview), in sync
     with the web export param names/order.
   - **Visual design:** page styling, consistent with the shared `gizza-chrome`. Original only.
   - **Drift-guard:** REGENERATE the `authored` schema literal in the block's drift-guard test
     to the new descriptor (do NOT keep the old). Record the before→after schema diff for the
     PR. Keep `manifest.json` `tool.*` consistent with the new `descriptor()`.

6. **Phase 5 — Re-test (the "did we improve it" gate).** Re-run the full matrix
   (reference.md §"Phase 5"): unit + drift-guard (regenerated) + wafer fixtures + Playwright
   page **incl. the query-param deep-link** + CLI smoke + `wafer build` / `wasm-pack` /
   generator / `solobase build`. Hard gates: pre-existing behavior tests stay GREEN; **each new
   capability ships with its own test**; API/CLI/query all pass post-edit. ≤3 fix attempts per
   failure, then escalate (Honesty gate).

7. **Phase 6 — Ship (review-only).** Commit; `git push -u origin feat/improve-<slug>`;
   `gh pr create` with the body template in reference.md §"Phase 6" (competitor-analysis
   summary · before→after schema diff · per-dimension change list · Phase-1 fixes ·
   out-of-model features considered · tested+results · limitations). Commit a
   `docs/checks/<YYYY-MM-DD>-improve-<slug>-competitor-analysis.md` snapshot. Run `/code-review`
   on the diff and post findings as a PR comment. **Do NOT merge.**

**Honesty gate:** if a build/test fails unrecoverably (≤3 attempts), STOP and report the
failure with the error — never open a "done" PR for a broken tool. If fewer than 5 real
competitors exist, say so and proceed with what's real. If a surface can't be headlessly
verified (gpu/imagine has no page; chat-ffmpeg can't run in a Service Worker — page + CLI
only), state it explicitly rather than claiming a pass.
````

- [ ] **Step 4: Run the structural check to verify it passes**

Run: `cd gizza-ai && bash .claude/skills/improve-tool/check-skill.sh`
Expected: `SKILL.md structural check: PASS`

- [ ] **Step 5: Remove the temp check and commit**

```bash
cd gizza-ai
rm .claude/skills/improve-tool/check-skill.sh
git add .claude/skills/improve-tool/SKILL.md
git commit -m "feat(improve-tool): SKILL.md — 6-phase autonomous improve procedure"
```

---

### Task 2: Author `reference.md`

**Files:**
- Create: `gizza-ai/.claude/skills/improve-tool/reference.md`

**Interfaces:**
- Consumes: `SKILL.md`'s phase references (`reference.md §"Phase N"`).
- Produces: the per-phase recipes the SKILL.md points at — Phase-1 verify commands, the Phase-2 parallel-research subagent prompt + competitor-profile schema, the Phase-4 per-dimension edit recipes + drift-guard-regenerate recipe (concrete `url-encode` example), the Phase-5 build/test matrix, and the Phase-6 PR + snapshot templates.

- [ ] **Step 1: Write the failing structural check**

Create `gizza-ai/.claude/skills/improve-tool/check-ref.sh` (temporary, deleted in Step 5):

```bash
#!/usr/bin/env bash
set -euo pipefail
F=.claude/skills/improve-tool/reference.md
fail=0
need() { grep -q "$1" "$F" || { echo "MISSING: $1"; fail=1; }; }
need 'Phase 1'; need 'gizza tool'; need 'cargo test --workspace'
need 'Phase 2'; need 'competitor-profile'; need 'paraphrase'
need 'Phase 4'; need 'drift-guard'; need 'authored'
need 'Phase 5'; need 'wasm-pack'; need 'solobase build'
need 'Phase 6'; need 'Out-of-model'
[ "$fail" = 0 ] && echo "reference.md structural check: PASS" || { echo "reference.md structural check: FAIL"; exit 1; }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd gizza-ai && bash .claude/skills/improve-tool/check-ref.sh`
Expected: FAIL (`reference.md` does not exist yet).

- [ ] **Step 3: Write `reference.md`**

Create `gizza-ai/.claude/skills/improve-tool/reference.md` with EXACTLY this content:

````markdown
# improve-tool — reference (per-phase recipes, commands, templates)

Each `blocks/<slug>/` and `tools/generator` are SEPARATE cargo workspaces → `cd` into the
dir; do NOT use `-p <crate>` from repo root. `wafer build` runs from INSIDE `blocks/<slug>/`.

## Phase 1 — verify the three surfaces

The descriptor (`src/lib.rs` `descriptor()`) single-sources the chat schema, CLI, page, and
URL query-params. Verify all three live, from a known-good baseline:

- **API (LLM/chat schema + invoke):**
  - `cd blocks/<slug> && cargo test --workspace` (core + block units, incl. the drift-guard).
  - Run the wafer fixtures: each `tests/*.json` is a `{"kind":"invoke","data":[…bytes…],"meta":[]}`
    payload — the block invoke path. Confirm they pass.
  - Confirm `descriptor()`/`info()` emits the tool schema (it's what the chat LLM sees).
- **CLI:** `cargo install --path cli --force`, then `gizza tool <slug> "<args>"` returns a
  correct result (same wasmi runtime + block + schema as chat).
- **Query params (page deep-link):** Playwright navigate `/tools/<slug>/?<param>=<value>` and
  assert the field pre-fills and the output computes. (The page reads descriptor params from
  the URL query string.)

If any fails: fix at root cause in `feat/improve-<slug>`, record under a "Phase-1 fixes" PR
section. If the breakage is far bigger than a focused fix → STOP and surface it.

## Phase 2 — competitor research (parallel)

1. `firecrawl-search` the tool's function (e.g. "url encoder online", "image resize online").
   Pick the **top 5** real competitor *tools* (a usable, reachable tool page — not a listicle
   or a login wall). If fewer than 5 real ones exist, say so and use what's real.
2. Dispatch **5 read-only subagents in parallel** (one per competitor URL). Subagent prompt:

   > You are researching ONE competitor tool for a gizza tool-improvement pass. Visit `<url>`
   > and return the competitor-profile JSON below. Use `firecrawl-scrape`/`firecrawl-extract`
   > for features/params and Playwright for a screenshot only if a picture beats markdown.
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

### drift-guard — REGENERATE the authored schema

The block has a unit test pinning `derived == authored` (e.g. url-encode's
`schema_json_matches_authored_chat_schema`), with `authored` as a hardcoded JSON literal. An
`/improve-tool` schema change is INTENTIONAL, so:

1. Run the test once to see the new derived schema in the failure diff, OR print it:
   `cd blocks/<slug> && cargo test schema_json_matches -- --nocapture` (read the assert diff).
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
- `solobase build` — rebuild app + blocks into `pkg/`.
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
````

- [ ] **Step 4: Run the structural check to verify it passes**

Run: `cd gizza-ai && bash .claude/skills/improve-tool/check-ref.sh`
Expected: `reference.md structural check: PASS`

- [ ] **Step 5: Remove the temp check and commit**

```bash
cd gizza-ai
rm .claude/skills/improve-tool/check-ref.sh
git add .claude/skills/improve-tool/reference.md
git commit -m "feat(improve-tool): reference.md — per-phase recipes, research prompt, templates"
```

---

### Task 3: Proof-run validation (the real test) + fix friction

**Files:**
- Read-only inputs: `gizza-ai/.claude/skills/improve-tool/{SKILL.md,reference.md}`
- Modify (if the proof exposes gaps): `gizza-ai/.claude/skills/improve-tool/{SKILL.md,reference.md}`
- Produced by the proof run (separate branch/PR): `feat/improve-url-encode`

**Interfaces:**
- Consumes: the two skill files from Tasks 1-2.
- Produces: a friction log → skill-file fixes; evidence the skill works end-to-end.

**Proof target:** `url-encode` — a pure tool, fully headlessly verifiable (unit + page + CLI), with real competitors (urlencoder.org and peers). Chosen so the proof exercises every phase without the chat-ffmpeg/gpu caveats.

- [ ] **Step 1: Dispatch a fresh zero-context subagent to execute the skill**

Dispatch a `general-purpose` subagent. Prompt (verbatim intent):

```
You have NO prior context. Read ONLY these two files and follow them exactly to improve the
gizza tool `url-encode`:
  gizza-ai/.claude/skills/improve-tool/SKILL.md
  gizza-ai/.claude/skills/improve-tool/reference.md
Execute all 6 phases on `url-encode`. Open the review-only PR; DO NOT merge. Then return a
FRICTION LOG: every point where the instructions were ambiguous, missing a command, or wrong —
quote the step and say what you needed that wasn't there. Be brutally specific.
```

- [ ] **Step 2: Verify the proof run against the pass criteria**

Confirm the subagent's PR + transcript show ALL of:
- Phase 1 ran the three surface checks (cargo test + fixtures, `gizza tool url-encode`, a `?` deep-link) and reported their status.
- Phase 2 found 5 real competitors via parallel subagents and returned paraphrased profiles (no verbatim copy).
- Phase 3 produced a gap list tagged in-model / out-of-model.
- Phase 4 made in-model edits AND regenerated the `authored` schema literal in `schema_json_matches_authored_chat_schema` (the test still passes).
- Phase 5: `cargo test --workspace` green, a new test per new capability, page+query Playwright + CLI exercised.
- Phase 6: a review-only PR (NOT merged) with the analysis table, schema diff, per-dimension changes, out-of-model list, and the `docs/checks/...-improve-url-encode-competitor-analysis.md` snapshot.

Expected: all present. If any is missing, that is a skill defect → Step 3.

- [ ] **Step 3: Fix every friction-log gap in the skill files**

For each friction-log item and each missing pass-criterion, edit `SKILL.md`/`reference.md` to close the gap (add the missing command, disambiguate the step, correct the recipe). Do NOT special-case `url-encode` — fixes must generalize to any tool.

- [ ] **Step 4: Re-verify the fix closes the gap**

If a fix changed a recipe materially, re-dispatch a fresh subagent on a DIFFERENT pure tool (`word-count`) with the same prompt, and confirm the friction is gone. (Skip re-dispatch only if the fixes were pure clarifications with no behavioral change — state which.)

Expected: no recurring friction; pass criteria met.

- [ ] **Step 5: Commit the skill fixes**

```bash
cd gizza-ai
git add .claude/skills/improve-tool/SKILL.md .claude/skills/improve-tool/reference.md
git commit -m "fix(improve-tool): close proof-run friction (url-encode shakedown)"
```

---

### Task 4: Ship the skill + update the handoff

**Files:**
- Modify: `gizza-ai/docs/superpowers/handoffs/2026-06-20-improve-tool-skill-handoff.md`
- (PR for branch `feat/improve-tool-skill`)

**Interfaces:**
- Consumes: the committed skill (Tasks 1-3).
- Produces: the merged-pending skill PR + an updated handoff marking the skill shipped.

- [ ] **Step 1: Mark the handoff shipped**

Append a `## Status 2026-06-20 — SHIPPED` section to `2026-06-20-improve-tool-skill-handoff.md` summarizing: skill at `.claude/skills/improve-tool/`, spec + plan paths, proof tool(s) used, and the proof-run PR number(s).

- [ ] **Step 2: Commit**

```bash
cd gizza-ai
git add docs/superpowers/handoffs/2026-06-20-improve-tool-skill-handoff.md
git commit -m "docs(improve-tool): mark skill shipped in handoff"
```

- [ ] **Step 3: Push and open the skill PR (review-only)**

```bash
cd gizza-ai
git push -u origin feat/improve-tool-skill
gh pr create --title "feat(improve-tool): autonomous /improve-tool skill" \
  --body "Adds the /improve-tool skill (SKILL.md + reference.md): verify+fix 3 surfaces → parallel 5-competitor research → comprehensive in-model improve → re-test → review-only PR. Spec docs/superpowers/specs/2026-06-20-improve-tool-skill-design.md; plan docs/superpowers/plans/2026-06-20-improve-tool-skill.md. Proof-run PR(s): <fill in>. No competitor copy/branding copied. 🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

- [ ] **Step 4: Code-review the skill diff**

Run `/code-review` on the `feat/improve-tool-skill` diff; post findings as a PR comment. Do NOT merge (matches the repo's PR-gate).

---

## Self-Review

**Spec coverage:**
- Input `/improve-tool <slug>` + one-tool-per-run → Task 1 SKILL phase 1. ✓
- Fully autonomous → review-only PR, never merge → Global Constraints + SKILL phases 6-7 + honesty gate. ✓
- 6 phases (verify/fix → research → diff → improve → re-test → ship) → Task 1 SKILL body + Task 2 reference recipes. ✓
- All 4 improvement dimensions → reference §Phase 4 + PR template. ✓
- No-copy/branding hard rule → Global Constraints + SKILL + Phase-2 subagent prompt + PR footer. ✓
- Fit-to-model filter / out-of-model list → SKILL + reference §Phase 3 + PR template. ✓
- Parallel 5-competitor research → SKILL phase 3 + reference §Phase 2 (5 subagents). ✓
- Drift-guard regenerates the authored schema → reference §Phase 4 drift-guard (concrete url-encode test) + PR schema diff. ✓
- Verify the 3 surfaces (API/CLI/query) → reference §Phase 1 commands. ✓
- Re-test gates (behavior green, new-cap test) → reference §Phase 5. ✓
- Competitor-analysis artifact (PR body + docs/checks snapshot) → reference §Phase 6. ✓
- Validation that the skill works → Task 3 proof run with pass criteria. ✓

**Placeholder scan:** the `<slug>`/`<param>`/`<url>`/`<fill in>` tokens are intentional template tokens inside the skill files / PR command, not plan TODOs. No "TBD/implement later". ✓

**Type/name consistency:** the drift-guard test name `schema_json_matches_authored_chat_schema`, the descriptor API (`Param::*`, `Input::*`, `to_schema_json`/`schema_json()`), and the command set match the real `blocks/url-encode/src/lib.rs` and `/new-tool`'s reference. Branch names `feat/improve-<slug>` (runs) vs `feat/improve-tool-skill` (the skill itself) are distinct and used consistently. ✓

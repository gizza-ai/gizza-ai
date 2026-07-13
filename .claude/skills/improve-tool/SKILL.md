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
   `git checkout -b feat/improve-<slug>` from `main` — UNLESS the tool isn't on main yet
   (freshly built on a tool-loop branch): then branch from THAT branch and pass it as the
   PR base (`gh pr create --base <branch>`) so the PR diff shows only the improvements.

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
   A third tag is allowed: **considered, rejected** — an in-model gap declined on judgment
   (schema bloat, UX regression, family-invariant conflict). State the reason in the PR;
   an honest rejection beats a forced feature (e.g. tag-list pills rejected for
   comma-bearing, bulk-pasted list fields). Classify capabilities honestly: if a competitor
   ships it as a FEATURE (e.g. a combination counter), it's a capability gap, not copy.

5. **Phase 4 — Improve (every in-model gap).** Edit per dimension (reference.md §"Phase 4"):
   - **Capabilities:** `core/src/lib.rs` logic + `src/lib.rs` `descriptor()` params (new
     options, better defaults, more formats, better output). `f64` not `i64` for numerics.
     For ffmpeg tools also update `web/src/lib.rs` `build_argv` + `page/meta.toml` field order.
   - **Copy/SEO:** `page/content.md` + `page/meta.toml` — original copy, examples, FAQ, tags,
     title/description.
   - **UX/layout:** the page's input/output presentation (grouping, presets, preview), in sync
     with the web export params. **Usability Standards** — the page must feel like a
     purpose-built tool, not a generated form:
     1. **Right control for the data.** Fixed choices are `Param::enumv` (renders a `<select>`);
        booleans are checkboxes; dates/times get native pickers; large standard vocabularies
        (timezones, currencies, country codes) get searchable autocomplete; list-valued fields
        get an add/remove UI, not a comma-separated text box.
     2. **Platform over per-tool hacks.** When a control kind or layout the tool needs doesn't
        exist yet, add it DECLARATIVELY to the shared generator
        (`tools/generator/src/{control,template}.rs` + `site/tool.js`) so every tool can use it.
        Never add another `cfg.slug === "…"` branch to the shared `site/tool.js` — that's the
        workspace fix-at-root-cause rule, and slug branches are why 6 tools' UI can't be reused.
     3. **Smart defaults + context detection.** Pre-fill what the browser already knows (today's
        date, the user's timezone via `Intl.DateTimeFormat().resolvedOptions().timeZone`) so the
        tool shows a result before the user types anything.
     4. **Worked examples everywhere.** Placeholders show a REAL input; the page copy shows at
        least one input→output pair a user can verify.
     5. **Layout stability.** Never resize/bounce the widget on errors, empty input, or
        keystrokes — nothing may jump under the user's cursor.
     6. **One-click reset** wherever inputs hold state worth clearing (restores defaults, not
        just blanks).
     7. **FAQ accordions.** `<details>`/`<summary>` with a blank line inside each so the answer
        renders as markdown and picks up the shared styling (hygiene-gated).
     8. **State the limits.** Max sizes, supported formats, depth caps, and edge-case behavior
        belong on the page — users shouldn't discover them via an error.
     9. **Errors say what was expected.** "expected X, got Y at Z" — never a bare "error"/
        "invalid input".
   - **Visual design:** page styling, consistent with the shared `gizza-chrome`. Original only.
   - **Drift-guard:** REGENERATE the `authored` schema literal in the block's drift-guard test
     to the new descriptor (do NOT keep the old). Record the before→after schema diff for the
     PR. Keep `manifest.json` `tool.*` consistent with the new `descriptor()`.

6. **Phase 5 — Re-test (the "did we improve it" gate).** Re-run the full matrix
   (reference.md §"Phase 5"): unit + drift-guard (regenerated) + wafer fixtures + Playwright
   page **incl. the query-param deep-link** + CLI smoke + `wafer build` / `wasm-pack` /
   generator / `impresspress build`. Hard gates: pre-existing behavior tests stay GREEN; **each new
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

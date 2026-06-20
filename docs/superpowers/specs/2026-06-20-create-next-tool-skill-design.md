# `/create-next-tool` skill — design

**Date:** 2026-06-20
**Status:** Design approved; implementation plan next.
**Siblings:** `/new-tool` (creates a tool from name+description) and `/improve-tool` (verifies +
competitor-improves an existing tool). `/create-next-tool` orchestrates both over a backlog CSV.

## Goal

Build the next un-built tool from `tools-to-build.csv`, end to end, on the current branch:
pick the next tool → build it with `/new-tool`'s procedure → fully enhance + verify it with
`/improve-tool`'s procedure → commit + push. One tool per run; re-invoke (or `/loop`) for the next.

## Locked decisions

- **No PRs, no new branch.** Works on the **current branch**; commits directly and pushes. (User
  override of the repo's usual PR rule, scoped to this bulk-build loop.)
- **Input:** none. Source of work is `tools-to-build.csv` in the gizza-ai repo root (1675 tools,
  demand-sorted; columns include `name`, `description`).
- **Next-tool selection:** walk the CSV **top-down** (already priority-sorted); `slug = kebab(name)`;
  take the **first row whose `blocks/<slug>/` is not committed in `git HEAD`**. (git-HEAD, not the
  working tree, so a half-scaffolded failure never counts as "built".)
- **Build depth:** **full** — `/new-tool` build steps, then the **entire** `/improve-tool` (verify
  the 3 surfaces + research top-5 competitors + close in-model gaps + re-test). Each new tool ships
  competitor-grade, not minimal.
- **Composition:** thin orchestrator that **references** the two skills' procedures and **skips their
  branch/PR/commit steps**, doing the git itself (DRY — the heavy logic stays in those skills).
- **One tool per run.**

## Architecture

```
/create-next-tool   (on the CURRENT branch — no branch, no PR)
        │
  ┌─────▼─────────────────────────────────────────────────────┐
  │ 1. PICK NEXT                                               │
  │    read tools-to-build.csv (root), top-down               │
  │    slug = kebab(name); first slug NOT in `git ls-tree HEAD │
  │    blocks/` → its name + description are the build inputs  │
  └─────┬─────────────────────────────────────────────────────┘
        │
  ┌─────▼─────────────────────────────────────────────────────┐
  │ 2. BUILD  (follow /new-tool steps 3–8)                     │
  │    classify type → scaffold-tool.sh → implement core/      │
  │    descriptor/web/page → build → test                     │
  │    SKIP /new-tool step 2 (branch) + steps 9–10 (PR/review)│
  └─────┬─────────────────────────────────────────────────────┘
        │
  ┌─────▼─────────────────────────────────────────────────────┐
  │ 3. IMPROVE  (follow /improve-tool Phases 1–5 on <slug>)    │
  │    verify 3 surfaces + fix → research top-5 → diff →       │
  │    close in-model gaps → re-test                          │
  │    SKIP its branch step + Phase 6 (PR)                    │
  │    writes docs/checks/<date>-improve-<slug>-…analysis.md  │
  └─────┬─────────────────────────────────────────────────────┘
        │
  ┌─────▼─────────────────────────────────────────────────────┐
  │ 4. COMMIT + PUSH  (current branch)                        │
  │    commit A: feat(<slug>): new tool                       │
  │    commit B: feat(<slug>): competitor improvements +       │
  │              analysis                                     │
  │    git push                                               │
  └───────────────────────────────────────────────────────────┘
```

## Phase detail

### 1. Pick next
- The backlog is `tools-to-build.csv` at the gizza-ai repo root. Each row has at least `name` and
  `description` (plus `category`, `effort`, `priority`, …). Rows are already sorted highest-priority
  first.
- `slug = name.lower()` with non-alphanumerics collapsed to `-` (kebab) — the same slugging
  `/new-tool` applies to a name.
- "Already built" = `blocks/<slug>/` appears in `git ls-tree -d --name-only HEAD blocks/`. Walk
  top-down and pick the **first** row whose slug is not built. That row's `name` + `description` are
  the inputs for the build.
- If every row is built, report "backlog complete" and stop.

### 2. Build (delegate to `/new-tool`'s procedure)
- Read `.claude/skills/new-tool/SKILL.md` + `reference.md` and follow **steps 3–8**: classify the
  type (pure / ffmpeg / network / gpu), derive the schema, `scripts/scaffold-tool.sh <slug> <type>`,
  implement `core`/`descriptor`/`web`/`page`, build (`wafer build`, `cargo test`, `wasm-pack`,
  generator), and the type-aware tests.
- **Skip** `/new-tool` step 2 (branch creation) and steps 9–10 (push/PR/code-review) — git is owned
  by step 4 here.

### 3. Improve (delegate to `/improve-tool`'s procedure)
- Read `.claude/skills/improve-tool/SKILL.md` + `reference.md` and run **Phases 1–5** on `<slug>`:
  verify the three surfaces (chat/LLM API, CLI, page query-params) and fix any breakage; research the
  top-5 competitors; diff + rank gaps (fit-to-model); close every in-model capability/copy/UX/visual
  gap; regenerate the drift-guard; re-run the full test matrix.
- **Skip** `/improve-tool`'s "Gather + branch" step and **Phase 6** (PR). The competitor-analysis
  snapshot is still written to `docs/checks/<YYYY-MM-DD>-improve-<slug>-competitor-analysis.md`.
- All of `/improve-tool`'s rules carry over: **never copy competitor copy/branding/trademarks**;
  out-of-model features listed not built.

### 4. Commit + push
- Stage and commit on the **current branch** (no PR):
  - `feat(<slug>): new tool` — the scaffolded + built tool (after step 2).
  - `feat(<slug>): competitor improvements + analysis` — the `/improve-tool` changes + the
    `docs/checks/` snapshot (after step 3).
- `git push` the current branch.

## Error handling / honesty gate

- Inherits both skills' honesty gates: ≤3 fix attempts per failure, then escalate.
- **If the build (step 2) or verification (step 3 Phase 1) fails unrecoverably:** STOP, run
  `git clean -fd blocks/<slug>` to remove the partial/uncommitted scaffold (so the slug is **not**
  left half-built — it must remain "not built" for a future retry), and report the failure with the
  error. **Never commit a broken tool** — a committed broken tool would be skipped forever by the
  next-tool check.
- If a surface can't be headlessly verified (gpu has no page; chat-ffmpeg can't run in a Service
  Worker), state it explicitly rather than claiming a pass (same as `/improve-tool`).
- Never claim a step passed that wasn't run.

## Components

- **`SKILL.md`** — the 4-step orchestration procedure + the honesty/cleanup gate. It is intentionally
  thin: it points at `/new-tool` and `/improve-tool` for the actual build/improve recipes and only
  owns next-selection + commit-to-current-branch.
- No `reference.md` needed — the recipes live in the two sibling skills. (Add one only if the
  next-selection or commit logic grows beyond a few commands.)

## Known limitation

Exact-slug matching: a semantic near-duplicate of an existing tool (e.g. `pdf-to-text` while
`pdf-extract-text` is built) can still be selected and built — this matches the user's "match by
folder name" choice. If the build/verify notices the tool duplicates an existing one, flag it in the
report rather than silently shipping a redundant tool.

## Out of scope (YAGNI)

- Multi-tool loops inside one run (one tool per run; use `/loop` for batches).
- A CSV `status`/`done` column (git-HEAD folder presence is the source of truth).
- PR creation / code-review posting (no PRs in this flow).
- Semantic de-duplication against existing tools (exact-slug only).

## References

- `.claude/skills/new-tool/{SKILL.md,reference.md}` — the build procedure (steps 3–8 used here).
- `.claude/skills/improve-tool/{SKILL.md,reference.md}` — the verify+improve procedure (Phases 1–5).
- `tools-to-build.csv` (repo root) — the demand-sorted backlog (1675 tools).

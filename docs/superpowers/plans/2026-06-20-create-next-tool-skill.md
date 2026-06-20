# `/create-next-tool` Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `/create-next-tool` gizza skill — pick the next un-built tool from `tools-to-build.csv`, build it with `/new-tool`'s procedure, fully enhance+verify it with `/improve-tool`'s procedure, then commit + push on the current branch (no PR).

**Architecture:** One thin prose file (`SKILL.md`) that orchestrates the two existing sibling skills. The only novel, deterministic logic is the next-tool selection (a CSV walk filtered by `git ls-tree HEAD blocks/`); everything else delegates to `/new-tool` and `/improve-tool`. Validation = a structural check on the file + a functional check that the embedded selection command picks the correct next tool against real data.

**Tech Stack:** Markdown skill file; the embedded selection helper is Python 3 (csv + `git ls-tree`); the build/improve recipes live in the sibling skills (Rust gizza toolchain, firecrawl, Playwright).

## Global Constraints

- **Skill location:** `gizza-ai/.claude/skills/create-next-tool/SKILL.md` — sibling to `new-tool/` and `improve-tool/`. No `reference.md` (recipes live in the sibling skills).
- **Runtime: no new branch, no PR.** Works on the **current branch**; commits directly + pushes. (Scoped user override of the repo PR rule.)
- **Next-tool selection:** walk `tools-to-build.csv` (repo root) **top-down**; `slug = kebab(name)`; pick the **first row whose `blocks/<slug>/` is NOT in `git ls-tree -d --name-only HEAD blocks/`**. `BACKLOG_COMPLETE` if all built.
- **Build depth:** `/new-tool` **steps 3–8**, SKIP its step 2 (branch) + steps 9–10 (PR/review).
- **Improve depth:** FULL `/improve-tool` **Phases 1–5** on `<slug>`, SKIP its branch step + **Phase 6** (PR). Writes `docs/checks/<YYYY-MM-DD>-improve-<slug>-competitor-analysis.md`.
- **No-copy rule (inherited):** NEVER copy competitor copy/branding/trademarks; list out-of-model features, don't build them.
- **Cleanup-on-failure:** if build/verify fails unrecoverably, STOP + `git clean -fd blocks/<slug>` (never commit a broken tool) + report.
- **One tool per run.**
- **Spec:** `docs/superpowers/specs/2026-06-20-create-next-tool-skill-design.md`.
- **PRs not direct-to-main** for developing the skill itself (this plan's work lands via branch `feat/create-next-tool-skill` + PR).

---

### Task 1: Author `SKILL.md` (orchestrator + selection logic)

**Files:**
- Create: `gizza-ai/.claude/skills/create-next-tool/SKILL.md`

**Interfaces:**
- Produces: the skill entrypoint — frontmatter `name: create-next-tool` + a `description` that triggers on "build the next gizza tool from the backlog"; a 4-step procedure (pick → build → improve → commit+push); the honesty/cleanup gate; an embedded Python selection helper that prints `<slug>\t<name>\t<description>` or `BACKLOG_COMPLETE`.
- Consumes: the sibling skills `.claude/skills/{new-tool,improve-tool}/` (referenced, not modified); `tools-to-build.csv` at repo root.

- [ ] **Step 1: Write the failing structural check**

Create `gizza-ai/.claude/skills/create-next-tool/check.sh` (temporary, deleted in Step 6):

```bash
#!/usr/bin/env bash
set -euo pipefail
F=.claude/skills/create-next-tool/SKILL.md
fail=0
need() { grep -qi "$1" "$F" || { echo "MISSING: $1"; fail=1; }; }
need "^name: create-next-tool$"
need "tools-to-build.csv"
need "git ls-tree"                 # git-HEAD selection
need "BACKLOG_COMPLETE"
need "new-tool"; need "improve-tool"
need "steps 3"                     # new-tool build steps
need "Phases 1"                    # improve-tool phases
need "git clean -fd blocks"        # cleanup-on-failure
need "current branch"
need "NEVER copy"                  # inherited no-copy rule
need "One tool per run"
[ "$fail" = 0 ] && echo "SKILL.md structural check: PASS" || { echo "FAIL"; exit 1; }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd gizza-ai && bash .claude/skills/create-next-tool/check.sh`
Expected: FAIL (SKILL.md does not exist yet).

- [ ] **Step 3: Write `SKILL.md`**

Create `gizza-ai/.claude/skills/create-next-tool/SKILL.md` with EXACTLY this content:

````markdown
---
name: create-next-tool
description: "Use when the user wants to build the next un-built gizza tool from the tools-to-build.csv backlog. Picks the next tool whose blocks/<slug>/ folder doesn't exist yet, builds it with the /new-tool procedure, fully enhances + verifies it with the /improve-tool procedure (competitor research + 3-surface checks), then commits and pushes on the current branch. No new branch, no PR. One tool per run."
---

# create-next-tool — build the next backlog tool end to end

Autonomous: build ONE tool per run from `tools-to-build.csv`, on the CURRENT branch, with NO
new branch and NO PR — just commits + a push. It orchestrates the two sibling skills: follow
`/new-tool`'s build steps, then the FULL `/improve-tool` procedure, but own the git yourself.
NEVER claim a step passed that you didn't run.

Read the two sibling skills for the actual recipes:
- `.claude/skills/new-tool/{SKILL.md,reference.md}` — the build procedure.
- `.claude/skills/improve-tool/{SKILL.md,reference.md}` — the verify+improve procedure.

Follow these steps in order:

1. **Pick the next tool.** From the gizza-ai repo root, find the first un-built backlog row
   (built = tools whose `blocks/<slug>/` is committed in `git HEAD`, so a half-built failure never
   counts):
   ```bash
   python3 - <<'PY'
   import csv, re, subprocess
   out = subprocess.run(["git","ls-tree","-d","--name-only","HEAD","blocks/"],
                        capture_output=True, text=True).stdout.split()
   built = {b.split("/",1)[1] for b in out if "/" in b}
   def slug(s): return re.sub(r'[^a-z0-9]+','-', s.strip().lower()).strip('-')
   with open("tools-to-build.csv", newline='') as f:
       for r in csv.DictReader(f):
           s = slug(r["name"])
           if s not in built:
               print(f"{s}\t{r['name']}\t{r['description']}")
               break
       else:
           print("BACKLOG_COMPLETE")
   PY
   ```
   Output is `<slug>\t<name>\t<description>` for the next tool, or `BACKLOG_COMPLETE`. If
   `BACKLOG_COMPLETE`, report it and stop. Otherwise the `name` + `description` are your build inputs.

2. **Build** — follow `/new-tool` **steps 3–8** (classify type → `scripts/scaffold-tool.sh <slug> <type>`
   → implement `core`/`descriptor`/`web`/`page` → build → type-aware tests) using the `name` +
   `description` from step 1. **SKIP** `/new-tool` step 2 (branch) and steps 9–10 (push/PR/code-review)
   — git is owned by step 4 here.

3. **Improve** — follow the FULL `/improve-tool` **Phases 1–5** on `<slug>`: verify the 3 surfaces
   (chat/LLM API, CLI, page query-params) + fix any breakage → research the top-5 competitors → diff +
   rank gaps (fit-to-model) → close every in-model capability/copy/UX/visual gap → regenerate the
   drift-guard → re-run the full test matrix. Write the competitor-analysis snapshot to
   `docs/checks/<YYYY-MM-DD>-improve-<slug>-competitor-analysis.md`. **SKIP** `/improve-tool`'s "Gather
   + branch" step and **Phase 6** (PR). Its rules carry over: **NEVER copy competitor
   copy/branding/trademarks**; list out-of-model features, don't build them.

4. **Commit + push** on the CURRENT branch (no PR). Two commits keep history clear:
   ```bash
   git add blocks/<slug> tests/
   git commit -m "feat(<slug>): new tool"
   git add blocks/<slug> tests/ docs/checks/
   git commit -m "feat(<slug>): competitor improvements + analysis"
   git push
   ```

**Honesty + cleanup gate:** if the build (step 2) or verification (step 3 Phase 1) fails
unrecoverably (≤3 fix attempts per the sibling skills), **STOP, run `git clean -fd blocks/<slug>` to
remove the partial scaffold, and report the failure with the error.** NEVER commit a broken tool — a
committed broken tool's `blocks/<slug>/` would make the next run skip it forever. If a surface can't
be headlessly verified (gpu has no page; chat-ffmpeg can't run in a Service Worker — page + CLI only),
state it explicitly rather than claiming a pass. **One tool per run**; re-invoke (or `/loop`) for the
next.

**Known limitation:** exact-slug matching — a semantic near-dup of an existing tool (e.g.
`pdf-to-text` while the built tool is `pdf-extract-text`) can still be picked. If you notice the tool
duplicates an existing one, flag it in your report rather than silently shipping a redundant tool.
````

- [ ] **Step 4: Run the structural check (PASS)**

Run: `cd gizza-ai && bash .claude/skills/create-next-tool/check.sh`
Expected: `SKILL.md structural check: PASS`

- [ ] **Step 5: Functionally verify the selection helper against real data**

Extract the Python selection block from the SKILL and run it from the repo root; it must pick the top un-built backlog tool and print three tab-separated fields.

Run:
```bash
cd gizza-ai
python3 - <<'PY'
import csv, re, subprocess
out = subprocess.run(["git","ls-tree","-d","--name-only","HEAD","blocks/"],
                     capture_output=True, text=True).stdout.split()
built = {b.split("/",1)[1] for b in out if "/" in b}
def slug(s): return re.sub(r'[^a-z0-9]+','-', s.strip().lower()).strip('-')
with open("tools-to-build.csv", newline='') as f:
    for r in csv.DictReader(f):
        s = slug(r["name"])
        if s not in built:
            print(f"{s}\t{r['name']}\t{r['description']}"); break
    else:
        print("BACKLOG_COMPLETE")
PY
```
Expected: one line, tab-separated, three fields, slug NOT among the built blocks — at the time of writing this is `pdf-to-text\tpdf-to-text\tExtracts the plain text content from a PDF.` (the top CSV row; `pdf-to-text` is not a built block — the built one is `pdf-extract-text`). Confirm: the slug printed has **no** matching `blocks/<slug>/` directory (`ls blocks/<slug> 2>/dev/null` is empty).

- [ ] **Step 6: Remove the temp check and commit**

```bash
cd gizza-ai
rm .claude/skills/create-next-tool/check.sh
git add .claude/skills/create-next-tool/SKILL.md
git commit -m "feat(create-next-tool): SKILL.md — backlog orchestrator (build + improve + commit)"
```

---

### Task 2: Ship the skill (push + review-only PR)

**Files:**
- (PR for branch `feat/create-next-tool-skill` — spec + plan + SKILL.md)

**Interfaces:**
- Consumes: the committed `SKILL.md` (Task 1) + the already-committed spec/plan on this branch.

- [ ] **Step 1: Push the branch**

```bash
cd gizza-ai
git push -u origin feat/create-next-tool-skill
```

- [ ] **Step 2: Open the PR**

```bash
cd gizza-ai
gh pr create --base main --head feat/create-next-tool-skill \
  --title "feat(create-next-tool): backlog orchestrator skill" \
  --body "Adds /create-next-tool: picks the next un-built tool from tools-to-build.csv (git-HEAD folder check, top-down), builds it via /new-tool's steps, fully enhances+verifies it via /improve-tool's Phases 1-5, then commits + pushes on the current branch (no PR at runtime). Spec docs/superpowers/specs/2026-06-20-create-next-tool-skill-design.md; plan docs/superpowers/plans/2026-06-20-create-next-tool-skill.md. Thin orchestrator — recipes live in the sibling skills. 🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

- [ ] **Step 3: Code-review the skill diff**

Run `/code-review` on the `feat/create-next-tool-skill` diff; post findings as a PR comment. The skill is prose + an embedded selection command (validated in Task 1 Step 5). Do NOT merge (leave for the user, matching how the sibling skills landed).

---

## Self-Review

**Spec coverage:**
- No PRs / current branch → Global Constraints + SKILL step 4. ✓
- Input = tools-to-build.csv, no args → SKILL step 1. ✓
- Next-tool selection via git-HEAD folder check, top-down, kebab slug, BACKLOG_COMPLETE → SKILL step 1 helper + Task 1 Step 5 functional check. ✓
- Build = /new-tool steps 3–8, skip 2 + 9–10 → SKILL step 2. ✓
- Improve = full /improve-tool Phases 1–5, skip branch + Phase 6 → SKILL step 3. ✓
- Competitor-analysis snapshot to docs/checks/ → SKILL step 3. ✓
- Two commits + push on current branch → SKILL step 4. ✓
- Cleanup-on-failure (git clean -fd blocks/<slug>), never commit broken → SKILL honesty gate. ✓
- No-copy rule inherited → SKILL step 3. ✓
- One tool per run → SKILL honesty gate. ✓
- Known limitation (semantic near-dup) → SKILL footer. ✓
- Thin orchestrator, no reference.md → one file, Task 1. ✓

**Placeholder scan:** `<slug>`/`<type>`/`<YYYY-MM-DD>`/`<name>`/`<description>` are intentional template tokens inside the skill content, not plan TODOs. No "TBD/implement later". ✓

**Type/name consistency:** the selection helper emits `<slug>\t<name>\t<description>` consistently in SKILL step 1 and the Task 1 Step 5 verification; `git ls-tree -d --name-only HEAD blocks/` + `b.split("/",1)[1]` slug extraction match between the SKILL and the test; branch name `feat/create-next-tool-skill` (developing the skill) is distinct from the runtime "current branch, no branch" behavior and used consistently. ✓

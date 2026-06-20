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

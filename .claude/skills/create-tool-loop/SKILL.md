---
name: create-tool-loop
description: "Use to run the autonomous gizza tool-building LOOP in sub-agent dispatch mode — keep building the next backlog tool, one fresh builder sub-agent at a time, committing+pushing each on the current branch, indefinitely. This is the orchestration layer over the per-tool `create-next-tool` skill: it owns dispatch, pacing, failure/limit back-off, and task-leak cleanup. Invoke it (e.g. via `/loop /create-tool-loop` or by following these steps) to resume the loop in one command. Builds ONE tool per builder, sequentially."
---

# create-tool-loop — autonomous sub-agent dispatch loop

Run the gizza backlog builder as a long-running loop. You are a **thin dispatcher**: each
iteration spawns ONE fresh `general-purpose` sub-agent that builds the next backlog tool
end-to-end and returns a single one-line result. The heavy build transcript stays inside the
sub-agent so the dispatcher's own context stays tiny and the loop can run for hours.

This skill is the orchestration layer. The per-tool build recipe lives in the sibling
**`.claude/skills/create-next-tool/SKILL.md`** and its **`references/`** files (wasm-safe
crates, page patterns, ops gotchas) — each builder reads those itself. Keep build-level
findings THERE (in the matching references/ file); keep dispatcher/operational findings in
this file's **Operational findings log** below, and append to it whenever you learn something
new (see "Self-update").

Working context (DERIVE at dispatch time — never hardcode; boxes and branches change between runs):
- **`<REPO>`** = the gizza-ai checkout root: `git rev-parse --show-toplevel` from the invocation cwd.
- **`<BRANCH>`** = `git branch --show-current`. No new branch, no PR — just commit + push to
  `<BRANCH>`. **If it is `main`, STOP and ask the user for a feature branch first** (workspace
  rule: PRs, not direct-to-main).
- **Concurrency:** probe with `nproc` + `free -g`. Default **sequential — one builder at a time**:
  even on a big box, parallel builders race on the shared `cargo install`/generator/git state,
  and a Rust release build peaks 1–2 GB/core. Sequential is the safe default everywhere.
- Substitute `<REPO>`, `<BRANCH>`, and `<TODAY>` (current date) into the BUILDER PROMPT at every
  dispatch — the sub-agent gets a fresh context and only knows what you paste.

## Dispatcher algorithm (per iteration)

1. **Pace with `/loop` (dynamic mode).** Run this skill under `/loop /create-tool-loop` so each
   turn re-enters here. The real wake signal is the builder's completion `<task-notification>`;
   the `ScheduleWakeup` you set is only a **fallback heartbeat** in case a build hangs.
2. **(First iteration / sanity)** Confirm branch and next tool:
   ```bash
   cd <REPO> && git branch --show-current   # must NOT be main — see Working context
   python3 scripts/pick-next-tool.py 2>/dev/null | grep -v '^skip' | head -1
   ```
   `pick-next-tool.py` prints `<slug>\t<name>\t<description>\t<type_hint>` or a sentinel
   (`BACKLOG_COMPLETE` / `NO_BUILDABLE_REMAINING`) — on a sentinel, report and stop the loop.
   `--stats` shows the backlog breakdown.
3. **Dispatch ONE builder:** `Agent(subagent_type:"general-purpose", description:"Build next gizza
   tool", prompt: <BUILDER PROMPT below>)`. Do NOT build inline. Do NOT run two builders at once.
4. **Set the fallback heartbeat:** `ScheduleWakeup` with `delaySeconds: 3000` (builds run ~6–60
   min; the completion notification re-invokes you sooner). `prompt` = the full `/loop` input
   verbatim so the next firing continues the loop.
5. **On the builder's completion notification:** read its one-liner.
   - `built+pushed <sha>` or `skiplisted <slug>` → continue: go to step 3 for the next tool.
   - `FAILED <slug>` or a rate-limit / usage / **529 Overloaded** error → **back off** (longer
     `ScheduleWakeup`, e.g. 1800–3600s), clean up the partial scaffold (see Operational findings),
     report, then retry the same pick next iteration.
6. **Report each result** to the user as a one-liner and keep a running tally
   (`built / skiplisted` counts). Keep dispatcher messages short.

## BUILDER PROMPT (canonical copy — paste verbatim, after substituting `<REPO>`/`<BRANCH>`/`<TODAY>`)

This is the ONLY copy of the builder prompt; `create-next-tool/SKILL.md` points here. Edit it
here or nowhere (a second copy WILL drift — it did once, re-introducing banned background builds).

> Build the next gizza backlog tool end-to-end. Working dir `<REPO>` (a gizza-ai checkout), branch
> `<BRANCH>`. Use absolute paths; `source $HOME/.cargo/env` in every bash command; `wafer build`
> runs from INSIDE blocks/<slug>/; cwd resets after /tmp commands.
>
> CRITICAL — NO BACKGROUND JOBS / NO TASK LEAKS: Run every build in the FOREGROUND with a generous
> timeout (the Bash tool supports timeout up to 600000 ms = 10 min; cargo test / wafer build /
> wasm-pack each finish well under that). Do NOT use run_in_background and do NOT spawn `sleep`/poll
> loops or `while` wait-loops — they leak as orphan background tasks. If for any reason you do start
> a background job, you MUST kill it before returning (e.g. `kill $(jobs -p) 2>/dev/null`). Do NOT
> end your turn while any build is still running. There is no external monitor.
>
> 1. **Pick:** `python3 scripts/pick-next-tool.py 2>/dev/null | grep -v '^skip' | head -1` →
>    `slug/name/description/type_hint`. On `BACKLOG_COMPLETE`/`NO_BUILDABLE_REMAINING`, report and stop.
> 2. **Dup check:** `ls blocks/ | grep -i <topic>`. A semantic dup of an existing block → append
>    `<slug>  # duplicate of blocks/<other>` to `docs/tool-skiplist.txt`, commit that, and return
>    `skiplisted <slug>: <reason>` (grep the named block's core/src/lib.rs to confirm first).
> 3. **Read the recipes:** `.claude/skills/create-next-tool/SKILL.md` plus its `references/` files
>    (wasm-safe crates, page patterns, ops gotchas) — they encode every dead end already hit.
> 4. **Competitor scan (BEFORE implementing):** one WebSearch for the tool's function; skim the top
>    3 real competitor tools (an unreachable one → replace it, don't run with fewer). List the
>    table-stakes params, defaults, worked examples, AND UX control patterns (sliders, color
>    pickers, preset buttons) ours must match, tag each in-model/out-of-model, and design the
>    descriptor to include the in-model ones from the start. Feasibility ≠ model fit: spike an
>    ffmpeg filter chain / wasm crate for 5 min before tagging a capability out-of-model. Every
>    table-stake ends in the descriptor OR the out-of-model list — never dropped silently. Record
>    the scan + decisions in
>    `docs/checks/<TODAY>-improve-<slug>-competitor-analysis.md` (paraphrase only — NEVER copy
>    competitor copy, branding, or trademarks; out-of-model items are listed, not built).
> 5. **Build:** classify type → `scripts/scaffold-tool.sh <slug> <type>` → implement core (≥1 happy
>    + ≥1 error unit test), descriptor (every param has `.describe()`; every fixed-choice param is
>    `Param::enumv`; drift-guard schema test), web export, page (`meta.toml` placeholders on every
>    text/number field; `content.md` with ≥1 worked example, ≥3 real `<details>` FAQs with a blank
>    line inside each, and stated limits/edge cases). Use the generator's CURRENT declarative
>    control kinds (slider/color/tag-list/date/time, `[input.labels]`, `[[example]]` preset chips —
>    see create-next-tool references/page-patterns.md) — the right control at build time, chips
>    whenever competitors ship presets. Fill EVERY scaffold TODO.
> 6. **Verify (all foreground):** `cargo test --workspace` (in blocks/<slug>/) → `wafer build` (in
>    blocks/<slug>/) → `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` →
>    `cargo install --path cli` → `python3 scripts/sync-tool-manifest.py <slug>` (generates
>    manifest.json tool.* from the live descriptor — never hand-edit those) → `cargo run
>    --manifest-path tools/generator/Cargo.toml -- .` → CLI (`gizza tool <slug> …`, incl. one
>    exact-output case) → Playwright (`cd tests && xvfb-run npx playwright test
>    tool-page-<slug>.spec.ts`). The page spec MUST assert real output — exact text for pure tools;
>    for media decode the result and assert dimensions/pixels (new-tool/reference.md §media
>    correctness) — plus one `?param=` deep-link case. Advertised-values matrix: one real run per
>    enum choice / accepted value form (short-hex AND long-hex — `#f00` once drew a white wave
>    while argv tests passed), ≥1 NON-default checkbox state, the exact cap boundary, ≥1 secondary
>    input format for media tools. Copy-paste-run the page's generated CLI example VERBATIM (pure:
>    succeeds; file tools: args parse + graceful fetch error).
> 7. **Hygiene gate:** `python3 scripts/check-tool-hygiene.py <slug>` must exit 0 (per-slug strict
>    mode also enforces placeholders, FAQ count, and meta description length).
> 8. **Honesty gate:** can't build+verify in ≤3 fix attempts → `git clean -fd blocks/<slug>`,
>    skiplist or report FAILED. NEVER commit broken.
> 9. **Cleanup:** `for d in blocks/*/target; do find "$d" -mindepth 1 -maxdepth 1 ! -name
>    block.wasm -exec rm -rf {} + ; done`. NEVER delete any web/pkg.
> 10. **Ship:** ONE commit for the tool + wasm artifacts + spec (a separate commit for the
>    competitor-analysis doc is fine); `git add -A && git commit && git push origin <BRANCH>`.
>
> Return ONE line only (no summary paragraphs): `<slug>: built+pushed <short-sha>` OR
> `skiplisted <slug>: <reason>` OR `FAILED <slug>: <reason>`.

Replace `<REPO>`, `<BRANCH>`, `<TODAY>` at every dispatch (the loop runs across midnight and
across machines/branches — stale substitutions have pushed to wrong branches before).

## Self-update — keep this skill improving

When you resolve a NOVEL dispatcher/operational issue (a new failure mode, a better pacing trick, a
cleanup gotcha), append a dated bullet to the **Operational findings log** below by Editing this
file. Build-level findings (a new wasm-safe crate, a page pattern, a dup) go in the matching
`create-next-tool/references/*.md` file instead, not here. Keep entries terse and actionable.

## Operational findings log (dispatcher layer)

**Sequential, context-isolated (2026-06-21).** One builder at a time. The win from sub-agents here
is CONTEXT, not speed: a fresh builder per tool keeps the dispatcher's context tiny so it runs for
hours. No races because only one builds at a time.

**FOREGROUND builds only — the #1 leak fix (2026-06-22).** Early builders ran heavy builds with
`run_in_background` + `sleep`/poll loops and didn't kill them before returning. Each finished
builder then left orphan poll-loop subshells that registered as perpetual "running" tasks — they
accumulated into 400–700+ stale task records and kept re-notifying ("came to rest" echoes). FIX:
the BUILDER PROMPT now mandates foreground builds (Bash timeout up to 600000 ms covers any single
step) and forbids background jobs. After this change, verified **0 new orphan processes** across
many tools. If you ever see the running-task count climb again, a builder is violating this rule.

**Stale re-notifications are normal — disregard them (2026-06-21).** A long-finished builder
re-fires `<task-notification>`s repeatedly as its leftover children wind down ("leftover polling
helper… no action needed. <slug>: built+pushed <sha>"). You already processed its one-liner;
ignore the echo. You CANNOT `TaskStop` a `completed` agent ("not running" error).

**Never clean+re-dispatch a builder that's merely "at rest" — it may resume and RACE (2026-06-21).**
One builder came to rest mid-build (no proper one-liner, waiting on a nonexistent monitor). I
cleaned its partial scaffold and dispatched a fresh builder for the same tool; the FIRST builder
then resumed and both built the same tool concurrently, overwriting each other and forcing a
`--force-with-lease`. Rule: a builder with no completion one-liner may still be alive. If a build
looks stalled, do NOT clean/redispatch on suspicion. First confirm it's truly idle (no `cargo`/
`rustc` procs AND no file activity for many minutes AND no completion), then `TaskStop` its agent
id, THEN verify git state, clean the partial scaffold, and redispatch. A slow build is common —
one legit build took ~58 min; don't mistake slow for stalled.

**Handling FAILED / 529 Overloaded (2026-06-22).** A 529 is a transient server overload; the
builder dies mid-build. The partial scaffold is uncommitted — clean it before retrying:
```bash
cd <REPO>
git clean -fd blocks/<slug>; rm -rf blocks/<slug> tests/tool-page-<slug>.spec.ts
git status -sb   # confirm working tree clean + branch in sync
```
Then back off with a longer `ScheduleWakeup` (1800–3600s) and re-dispatch; the picker returns the
same un-built slug, so it's retried, not skipped.

**Task-leak cleanup (only if the count climbs) (2026-06-22).** Stale records are on-disk
`.output` files under the session tasks dir (`/tmp/claude-<uid>/<sanitized-cwd>/<session>/tasks/`
— path varies per machine/session): `a*.output`
= agent transcripts (KEEP — back resume/notifications), `b*.output` = background-bash command
output. `TaskList` shows the live structured registry (usually empty here) and `TaskStop` returns
"not found" for these finished records — they're inert (no live process), just UI clutter. To trim
safely, delete only OLD background-bash files, protecting the active builder's recent ones:
```bash
TD=<session-tasks-dir>   # see path shape above
find "$TD" -name 'b*.output' -mmin +15 -delete   # never a*.output, never <15min files
```
With the foreground-build fix this should rarely be needed. If live orphan processes ever exist
(they shouldn't), kill stray `sleep [0-9]`/cargo/rustc ONLY when no builder is active — never while
a builder is mid-build (you'd kill its real compile). Beware: `pkill -f 'sleep [0-9]'` can match
your own script's later `sleep`; avoid `sleep` in the same command.

**Borderline skiplists — spot-check (2026-06-22).** Builders auto-skiplist semantic dups. Most are
right (e.g. `word-frequency-counter` = `word-frequency`). A few look wrong but are correct on
inspection — e.g. `toml-to-json` IS a legit dup of `json-yaml-convert` (that block converts
JSON/YAML/TOML any direction; it has a `toml_to_json` test). When a skiplist reason looks shaky,
`grep` the named existing block's `core/src/lib.rs` to confirm before trusting it; don't reflexively
un-skiplist.

**Picker wraps the alphabet; backlog is large (2026-06-22).** After the alphabetical tail, the
picker loops back to earlier deferred entries. As of 2026-06-22 the backlog was ~1675 total /
~205 built / ~1231 buildable — months of runway. Don't assume "near done" from the prompt's tool
count.

**Pacing / cache note.** `ScheduleWakeup ~3000s` is the fallback; the completion notification wakes
you sooner. Don't poll for the builder — it's harness-tracked and re-invokes you on completion.

**A dead builder can't be nudged — SendMessage queues forever (2026-07-02).** One builder died
silently mid-build (~50 min in, no completion notification, no error). Confirmed truly idle (no
cargo/rustc procs twice ~20 min apart, zero file writes for 70+ min), then tried
`SendMessage(to: <agent-id>, "resume…")` before the stop-clean-redispatch path, hoping to preserve
its partial work. The message only delivers "at its next tool round" — a dead agent never has one,
so the nudge is a no-op. Don't spend a heartbeat on it: once truly-idle is confirmed, go straight
to `TaskStop` → restore any modified shared files (`git checkout -- <file>`; builders may edit
`block-utils/`) → remove the untracked scaffold/docs → redispatch. The TaskStop notification shows
the builder's last step, which confirms where it died.

**Liveness = transcript mtime via `stat -L`, NOT repo-file activity (2026-07-02).** A builder can
legitimately go 50+ min with ZERO repo writes (long recipe-reading/research phase) while being
perfectly healthy. The reliable liveness probe is its transcript:
`stat -L -c 'size=%s mtime=%y' <session-tasks-dir>/<agent-id>.output` — growing size / recent
mtime = alive (every LLM turn appends). CRITICAL: the `.output` path is a SYMLINK; plain `stat`
returns the symlink's own size (~156 bytes, mtime = dispatch time) and reads as "died instantly" —
a false positive that nearly caused a wrongful stop-clean-redispatch. Also fine to `tail -c 1500`
the transcript for the current step — just never Read/cat the whole file (300KB+). Check the
transcript FIRST, before any repo-activity heuristics. Frozen size+mtime across two heartbeats
(~30 min) = dead, full stop — a live agent appends every turn. Two builders died silently this way
in one evening (mid-Bash-call, no error notification — likely terminal API errors); the fix is
just TaskStop → clean → redispatch, and the elapsed detection time already serves as the back-off.

**Commit dispatcher SKILL.md edits BEFORE dispatching the next builder (2026-07-02).** Builders
ship with `git add -A` at repo root — any uncommitted findings-log edit you've made gets silently
swept into the next tool's commit. Self-update this file, then commit it immediately (own commit,
`chore(skills): …`), then dispatch.

**Explicit-pick dispatches work; restate the one-liner rule when customizing (2026-07-03).** For
curated variety (image/video/audio slots), replace BUILDER PROMPT steps 1–2 with the given
slug/name/description/type plus targeted dup-check hints (name the suspect sibling blocks and any
skiplist lines that point AT this slug, e.g. "audio-waveform → build waveform-image instead" is
this build, not a dup). Two drift risks when customizing: builders return summary paragraphs
unless the return-line rule says "(no summary paragraphs)", and they may split wasm artifacts
into a fixup commit unless step 10 says ONE commit.

**Stacked build→improve runs: merge platform pieces forward and ENUMERATE them (2026-07-03).**
When improve passes land shared generator/site controls mid-run (slider, color kind,
`[input.labels]`, example chips, download-for-text), merge each improve PR into the working
branch before the next dispatch and list the now-available pieces in every subsequent prompt —
otherwise later agents re-invent them or ship pages that ignore them (the vignette build missed
the just-merged slider/labels kinds). Improve PRs based on the working branch (not main) keep
their diffs reviewable; the working branch is still user-reviewed before main.

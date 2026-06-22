---
name: create-tool-loop
description: "Use to run the autonomous gizza tool-building LOOP in sub-agent dispatch mode — keep building the next backlog tool, one fresh builder sub-agent at a time, committing+pushing each on the current branch, indefinitely. This is the orchestration layer over the per-tool `create-next-tool` skill: it owns dispatch, pacing, failure/limit back-off, and task-leak cleanup. Invoke it (e.g. via `/loop /create-tool-loop` or by following these steps) to resume the loop in one command. Builds ONE tool per builder, sequentially."
---

# create-tool-loop — autonomous sub-agent dispatch loop

Run the gizza backlog builder as a long-running loop. You are a **thin dispatcher**: each
iteration spawns ONE fresh `general-purpose` sub-agent that builds the next backlog tool
end-to-end and returns a single one-line result. The heavy build transcript stays inside the
sub-agent so the dispatcher's own context stays tiny and the loop can run for hours.

This skill is the orchestration layer. The per-tool build recipe and all crate/wasm build-level
findings live in the sibling **`.claude/skills/create-next-tool/SKILL.md`** — each builder reads
that itself. Keep build-level findings (wasm-safe crates, page patterns, etc.) THERE; keep
dispatcher/operational findings in this file's **Operational findings log** below, and append to
it whenever you learn something new (see "Self-update").

Working context (defaults; override if the user says otherwise):
- Working dir: `/root/gizza-ai/gizza-ai`
- Branch: `feat/tool-creating` (no new branch, no PR — just commit + push)
- Box: **2 CPU / ~3.9 GB RAM**. **Sequential only — one builder at a time** (parallel Rust release
  builds OOM and contend on the shared `cargo install`/generator/git state; 2 cores mean
  parallelism barely helps wall-clock anyway).

## Dispatcher algorithm (per iteration)

1. **Pace with `/loop` (dynamic mode).** Run this skill under `/loop /create-tool-loop` so each
   turn re-enters here. The real wake signal is the builder's completion `<task-notification>`;
   the `ScheduleWakeup` you set is only a **fallback heartbeat** in case a build hangs.
2. **(First iteration / sanity)** Confirm branch and next tool:
   ```bash
   cd /root/gizza-ai/gizza-ai && git branch --show-current
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

## BUILDER PROMPT (self-contained — paste verbatim to each sub-agent)

> Build the next gizza backlog tool end-to-end. Working dir /root/gizza-ai/gizza-ai; `source
> $HOME/.cargo/env` in every bash command; use absolute paths; for `wafer build` cd into
> blocks/<slug>/ first; cwd resets to /root after /tmp commands.
>
> CRITICAL — NO BACKGROUND JOBS / NO TASK LEAKS: Run every build in the FOREGROUND with a generous
> timeout (the Bash tool supports timeout up to 600000 ms = 10 min; cargo test / wafer build /
> wasm-pack each finish well under that). Do NOT use run_in_background and do NOT spawn `sleep`/poll
> loops or `while` wait-loops — they leak as orphan background tasks. If for any reason you do start
> a background job, you MUST kill it before returning (e.g. `kill $(jobs -p) 2>/dev/null`). Do NOT
> end your turn while any build is still running. There is no external monitor.
>
> Steps: (1) `python3 scripts/pick-next-tool.py 2>/dev/null | grep -v '^skip' | head -1`. (2) Read
> `.claude/skills/create-next-tool/SKILL.md` (esp. the Findings log) and follow the new-tool build
> procedure: classify type, `scripts/scaffold-tool.sh <slug> <type>`, implement core (+unit
> tests)/descriptor (+drift-guard schema test)/web/page, build in the foreground (`cargo test`, then
> `wafer build` in blocks/<slug>/ and `wasm-pack build blocks/<slug>/web --target web --release
> --out-dir pkg`), `cargo install --path cli`, `cargo run --manifest-path tools/generator/Cargo.toml
> -- .`, verify all applicable surfaces (CLI `gizza tool <slug>`; page Playwright `cd tests &&
> xvfb-run npx playwright test tool-page-<slug>.spec.ts`). (3) Write
> `docs/checks/<TODAY>-improve-<slug>-competitor-analysis.md` (use today's actual date). (4) If it's a
> semantic dup of an existing block, instead append `<slug>  # reason` to `docs/tool-skiplist.txt`,
> commit that, and re-pick. (5) Honesty gate: if it can't be built+verified in ≤3 fix attempts,
> `git clean -fd blocks/<slug>`, skiplist or report, do NOT commit broken. (6) Clean per-block target
> dirs (`for d in blocks/*/target; do find "$d" -mindepth 1 -maxdepth 1 ! -name block.wasm -exec rm
> -rf {} + ; done`), NEVER delete web/pkg. (7) `git add -A && git commit && git push origin
> feat/tool-creating`. Return ONE line only: `<slug>: built+pushed <short-sha>` OR `skiplisted
> <slug>: <reason>` OR `FAILED <slug>: <reason>`.

Replace `<TODAY>` with the current date each iteration (the loop runs across midnight).

## Self-update — keep this skill improving

When you resolve a NOVEL dispatcher/operational issue (a new failure mode, a better pacing trick, a
cleanup gotcha), append a dated bullet to the **Operational findings log** below by Editing this
file. Build-level findings (a new wasm-safe crate, a page pattern, a dup) go in
`create-next-tool/SKILL.md` instead, not here. Keep entries terse and actionable.

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
cd /root/gizza-ai/gizza-ai
git clean -fd blocks/<slug>; rm -rf blocks/<slug> tests/tool-page-<slug>.spec.ts
git status -sb   # confirm working tree clean + branch in sync
```
Then back off with a longer `ScheduleWakeup` (1800–3600s) and re-dispatch; the picker returns the
same un-built slug, so it's retried, not skipped.

**Task-leak cleanup (only if the count climbs) (2026-06-22).** Stale records are on-disk
`.output` files under the session tasks dir (`/tmp/claude-0/-root/<session>/tasks/`): `a*.output`
= agent transcripts (KEEP — back resume/notifications), `b*.output` = background-bash command
output. `TaskList` shows the live structured registry (usually empty here) and `TaskStop` returns
"not found" for these finished records — they're inert (no live process), just UI clutter. To trim
safely, delete only OLD background-bash files, protecting the active builder's recent ones:
```bash
TD=/tmp/claude-0/-root/<session-id>/tasks
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

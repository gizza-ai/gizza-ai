## What this tool does

A **runbook** is a how-to document whose steps are fenced code blocks: install
this, migrate that, restart the service. Paste one and this tool *tangles* it —
it pulls out every executable block, names it, numbers it, and joins the whole
thing into **one script you can actually run**, with an ordered task list in the
header. It runs entirely in your browser with WebAssembly: nothing is uploaded,
it works offline, and there's no sign-up.

The extractor:

- gives every block a **name** from its fence info string —
  `` ```bash "Install deps" ``, `` ```bash name=install-deps ``,
  `` ```bash title="Install deps" ``, `` ```bash id=… ``, or a Pandoc/Entangled
  attribute block `` ```{.bash #install-deps} ``,
- falls back to a **bold label** on the line above, then the **nearest heading**,
  then `step-N`, so untagged runbooks still produce a readable task list,
- **strips shell prompts** (`$ `, `% `, `❯ `, `PS C:\>`, `>>> `) and drops the
  un-prompted lines — the command *output* people paste along with the command,
- keeps **backslash continuations** attached to their command,
- **comments out** blocks tagged `skip`, `no-run`, `noexec`, `ignore`,
  `example` or `output` instead of running them, so a "don't do this" snippet
  can't be executed by accident,
- filters by **tag** (`` ```bash#deploy ``, `` ```bash deploy slow ``,
  `` ```{.bash .deploy} ``) with `-tag` to exclude, and
- records the **source line** of every step, so a failure points back into the
  document.

## Options

| Option | What it does |
| --- | --- |
| **Code blocks to extract** | `auto` (default) picks the language family with the most blocks. Or force `shell` (bash/sh/zsh/console/shell-session), `python` (python/py/pycon), `powershell` (powershell/pwsh/ps1), `javascript` (js/node), or `any` — every fence that carries a language tag. Untagged fences are never steps. |
| **Output** | `script` (default) — one runnable script whose header lists the ordered tasks. `tasks` — a Markdown checklist. `json` — `{language, count, runnable, steps:[…]}` with each step's code, tags and source line. |
| **Tag filter** | Comma-separated. A block is kept if it carries **any** listed tag; prefix with `-` to exclude (`deploy,-slow`). Empty keeps everything. |
| **Strip $ prompts** | On by default. Turn it off to keep a transcript verbatim. |
| **Add ==> progress lines** | On by default. Emits `echo "==> [2/5] Run migrations"` (or `print` / `Write-Host`) before each step. |
| **Abort-on-error header** | On by default. `set -euo pipefail` for shell, `$ErrorActionPreference = 'Stop'` for PowerShell. No-op for Python and JavaScript, which already abort on an unhandled error. |
| **Comment out skip-tagged blocks** | On by default. Turn it off to make them runnable. |

## Worked example

Given this runbook:

````markdown
# Deploy the API

## Install dependencies

```console
$ npm ci
added 42 packages
```

## Run migrations

```bash name=migrate
./manage.py migrate
```

## Rollback (do not run)

```bash skip
./manage.py migrate --rollback
```
````

the default **script** output is:

```bash
#!/usr/bin/env bash
# Runbook: 2 runnable task(s) of 3 extracted from Markdown.
# Tasks:
#   1. Install dependencies
#   2. migrate
#   3. Rollback (do not run)  [skipped: tagged skip]

set -euo pipefail

# --- 1/3 · Install dependencies (console, line 5) ---
echo "==> [1/3] Install dependencies"
npm ci

# --- 2/3 · migrate (bash, line 12) ---
echo "==> [2/3] migrate"
./manage.py migrate

# --- 3/3 · Rollback (do not run) (bash, line 18) — SKIPPED, tagged skip ---
# ./manage.py migrate --rollback
```

Note what happened: the `$ ` prompt is gone, `added 42 packages` (command
*output*, not a command) was dropped, step 1 took its name from the heading and
step 2 from `name=migrate`, and the rollback block is commented out because it
carries the `skip` tag.

Switch **Output** to `tasks` for the same runbook and you get a checklist:

```markdown
# Runbook tasks (2 runnable of 3)

- [ ] 1. Install dependencies — `console`, 1 line, line 5
- [ ] 2. migrate — `bash`, 1 line, line 12
- [ ] 3. ~~Rollback (do not run)~~ — `bash`, 1 line, line 18, tags: skip — skipped, tagged skip
```

## Limits & edge cases

- **Input cap: 1,000,000 characters** and **500 matching code blocks** per run.
  Past either you get a clear error — split the document or narrow it with a tag
  filter.
- **Untagged fences are never steps.** A bare ` ``` ` block is almost always
  sample output or a config snippet, so it's ignored. Tag it (`` ```bash ``) to
  include it.
- **Empty fences are skipped** and don't take a step number.
- **It never executes anything.** The output is text you review and run
  yourself — read it before you do, especially anything tagged `skip`.
- **Prompt stripping is all-or-nothing per block.** If any line in a block
  carries a prompt, only the prompted lines survive; if none does, the block is
  copied verbatim. That's what makes a pasted `console` session runnable without
  mangling ordinary scripts.
- **Nested fences work** as long as the outer fence is longer — use ` ````bash `
  around a block that itself contains ` ``` ` (e.g. a heredoc writing Markdown).
- Steps run **top to bottom in document order**; there's no dependency graph and
  no `<<macro>>` expansion between named blocks.
- The output has **no trailing newline** — copy or download it as-is.

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes. Extraction runs locally in your browser with WebAssembly — your runbook
never leaves your device, and the page keeps working offline once it has loaded.
There's no account and nothing is uploaded.

</details>

<details>
<summary>How do I name a step?</summary>

Put the name in the fence's info string. All of these work:
`` ```bash "Install deps" ``, `` ```bash name=install-deps ``,
`` ```bash title="Install deps" ``, `` ```bash id=install ``, and the
Pandoc/Entangled attribute form `` ```{.bash #install-deps} ``. If you don't name
a block, the tool uses a bold label directly above it, then the nearest heading,
then `step-N`.

</details>

<details>
<summary>Why did my command output end up missing?</summary>

That's **Strip $ prompts** doing its job. In a block where at least one line
starts with a prompt (`$ `, `% `, `❯ `, `PS C:\>`, `>>> `), the un-prompted lines
are treated as the command's output and dropped, and the prompts themselves are
removed — which is exactly what turns a copied README session into runnable
commands. Turn the option off to keep the transcript verbatim.

</details>

<details>
<summary>How do I stop a dangerous snippet from ending up in the script?</summary>

Tag its fence: `` ```bash skip `` (or `no-run`, `noexec`, `ignore`, `example`,
`output`). Tagged blocks still appear in the task list and in the script — but
commented out, with a `SKIPPED` marker — so nothing is silently lost and nothing
runs by accident. Alternatively, filter them out entirely with a `-` tag filter.

</details>

<details>
<summary>Can I extract only part of a runbook?</summary>

Yes, two ways. Tag your fences (`` ```bash#deploy ``, `` ```bash deploy slow ``,
or `` ```{.bash .deploy} ``) and put those tags in the **Tag filter** —
`deploy,-slow` keeps deploy steps and drops slow ones. Or switch **Code blocks to
extract** to a single language so only, say, the Python blocks come through.

</details>

<details>
<summary>Does it work for Python, PowerShell or Node runbooks?</summary>

Yes. Set **Code blocks to extract** (or leave it on auto, which picks whichever
family has the most blocks). Each family gets the right script shape: a
`#!/usr/bin/env python3` or `node` shebang, `$ErrorActionPreference = 'Stop'` for
PowerShell, and progress lines rendered as `print`, `console.log` or `Write-Host`
rather than `echo`. Python `>>>` doctest prompts are stripped too.

</details>

<details>
<summary>What's the JSON output for?</summary>

Automation. It returns `{language, count, runnable, steps:[…]}` where each step
carries its `index`, `name`, `language`, source `line`, `tags`, `skipped` flag,
line count and raw `code` — enough to drive your own runner, generate a ticket
per step, or diff a runbook between two revisions.

</details>

# improve-tool competitor analysis — `markdown-runbook-extractor`

Date: 2026-08-17 · scope: build + first improve pass, run together (new tool).

Everything below is **paraphrased** from public documentation. No competitor copy,
branding, assets or trademarks were copied into this repo.

Only **three** genuinely comparable, reachable tools exist for "extract the executable
code blocks of a Markdown runbook" — the rest of the field is either a *runner*
(executes blocks, no extraction artifact) or a general Markdown query tool. Per the
skill's honesty gate: 3 real competitors, profiled below, rather than padding to 5.

## Profiles

### 1. runbook (khalidx/runbook)

```json
{
  "name": "runbook",
  "url": "https://github.com/khalidx/runbook",
  "features": [
    "discovers *.md files in a directory and parses their fenced blocks",
    "a block counts as a command only when it has BOTH a language tag and a quoted name",
    "lists discovered commands, runs one by name, or serves them over HTTP",
    "handlebars templating for command arguments; commands can call other commands",
    "a command body can reference an external file instead of embedding code",
    "uniqueness check on name + arity; suggests near matches on a typo"
  ],
  "params_options": [
    {"name": "ls", "type": "subcommand", "default": "-", "range": "list discovered commands"},
    {"name": "run <command>", "type": "subcommand", "default": "-", "range": "execute one named command"},
    {"name": "serve", "type": "subcommand", "default": "-", "range": "web view of the docs + commands"}
  ],
  "input_formats": ["Markdown files on disk"],
  "output_formats": ["terminal execution", "command listing", "web page"],
  "output_quality": "no assembled-script artifact — it executes, it does not tangle",
  "ux_patterns": ["named commands as the unit of work", "list-then-run", "typo suggestions", "local web view"],
  "seo_copy_angles": ["executable markdown documents", "share runbooks as docs", "templated commands"],
  "limits": ["unnamed or untagged blocks are ignored entirely", "requires a Node/Deno install"],
  "free_vs_paid": "free, open source"
}
```

### 2. rundoc (eclecticiq/rundoc)

```json
{
  "name": "rundoc",
  "url": "https://github.com/eclecticiq/rundoc",
  "features": [
    "collects fenced blocks and runs them in document order",
    "the fence's first tag names the interpreter; extra hash-separated tags categorise the block",
    "special block roles for environment variables and for secrets (secrets kept out of saved output)",
    "step numbering with start-at, breakpoint, retry and inter-step pause controls",
    "interactive prompting: for variables, before each block, or to edit a failed block",
    "each block runs in its own interpreter session unless a single-session flag is passed"
  ],
  "params_options": [
    {"name": "--tags / -t", "type": "list", "default": "all", "range": "run blocks carrying ANY listed tag"},
    {"name": "--must-have-tags / -T", "type": "list", "default": "-", "range": "require all listed tags"},
    {"name": "--must-not-have-tags / -N", "type": "list", "default": "-", "range": "exclude blocks with these tags"},
    {"name": "--step / -s", "type": "int", "default": "1", "range": "start at step N"},
    {"name": "--breakpoint / -b", "type": "int", "default": "-", "range": "pause at step N"},
    {"name": "--retry / -r", "type": "int", "default": "0", "range": "retries per failed step"},
    {"name": "--output / -o", "type": "path", "default": "-", "range": "write a JSON session record"}
  ],
  "input_formats": ["Markdown files"],
  "output_formats": ["live terminal session", "JSON run record (code, interpreter, tags, per-run output + exit code + timings)"],
  "output_quality": "rich structured run record; again a runner, not an extractor",
  "ux_patterns": ["tag-based filtering", "numbered steps with breakpoints", "secret-aware blocks", "resume from step N"],
  "seo_copy_angles": ["run your documentation", "documentation as automation", "reproducible procedures"],
  "limits": ["blocks with no highlight tag are ignored", "no stated size limits", "Python install required"],
  "free_vs_paid": "free, open source"
}
```

### 3. codedown (earldouglas/codedown)

```json
{
  "name": "codedown",
  "url": "https://github.com/earldouglas/codedown",
  "features": [
    "reads Markdown on stdin and writes the blocks of one language to stdout",
    "a wildcard selects blocks of any language",
    "an optional separator string is inserted between concatenated blocks",
    "can restrict extraction to one section, addressed by heading number (e.g. 1.3.1) or heading text"
  ],
  "params_options": [
    {"name": "<language>", "type": "positional", "default": "-", "range": "language tag to extract, or a wildcard"},
    {"name": "--separator", "type": "string", "default": "none", "range": "text inserted between blocks"},
    {"name": "--section", "type": "string", "default": "whole doc", "range": "heading number or heading text"}
  ],
  "input_formats": ["Markdown on stdin"],
  "output_formats": ["concatenated code on stdout"],
  "output_quality": "raw concatenation — no names, no numbering, no prompt handling, no manifest",
  "ux_patterns": ["unix pipe (`codedown bash | bash`)", "section addressing by heading"],
  "seo_copy_angles": ["test the code in your README", "extract code blocks from markdown", "literate programming"],
  "limits": ["no block naming or provenance", "no stated size limits", "CLI only"],
  "free_vs_paid": "free, open source"
}
```

Also surveyed and rejected as non-comparable: mdrb, markdown-code-runner and
Markdown Exec (runners with no extraction artifact); Entangled, lmt, md-tangle and
mdsh (literate-programming tanglers that write *source files* via `<<macro>>`
reference expansion, a different job); `get-code-from-markdown` / `exdown` /
`codeblocks` (thin extract-for-testing helpers, a strict subset of codedown).

## Gap analysis → what shipped

| # | Gap (≥1 competitor has it) | Dimension | Verdict | Where it landed |
| - | -------------------------- | --------- | ------- | --------------- |
| 1 | Quoted-name blocks (`` ```bash "Install deps" ``) as the unit of work | capabilities | **in-model, built** | `parse_info` accepts the quoted form |
| 2 | `name=` / `title=` / `id=` attributes, plus Pandoc/Entangled `{.bash #id}` | capabilities | **in-model, built** | `parse_info`; test `names_come_from_every_supported_info_string_dialect` |
| 3 | Hash-separated tags on the fence (`` ```bash#deploy#v2 ``) | capabilities | **in-model, built** | `parse_info` splits the lang token on `#` |
| 4 | Tag filtering, include **and** exclude (rundoc `-t` / `-N`) | capabilities | **in-model, built** | `tags` param, `deploy,-slow` syntax; `TagFilter` |
| 5 | Step numbering + a listing of discovered commands (`runbook ls`) | capabilities/UX | **in-model, built** | task-manifest header in script mode + the `tasks` output format |
| 6 | Structured JSON run record with per-block interpreter/tags/code | capabilities | **in-model, built** | `output=json`, plus a `line` field no competitor exposes |
| 7 | Interpreter chosen per language tag; several families supported | capabilities | **in-model, built** | `language` param + per-family shebang, comment char, echo form |
| 8 | Only tagged blocks count; untagged fences ignored | capabilities | **in-model, built** | untagged fences are never steps (documented on the page) |
| 9 | Separator/annotation between concatenated blocks (codedown) | copy/UX | **in-model, built** | per-step `# --- i/n · name (lang, line N) ---` banner |
| 10 | Skip/do-not-run semantics for illustrative snippets | capabilities | **in-model, built** (none of the three has this) | `skip_marked` + `SKIP_TAGS`, commented out rather than dropped |
| 11 | Prompt stripping for pasted `console` sessions | capabilities | **in-model, built** (none of the three has this) | `strip_prompts`, incl. `>>>` doctest and `PS C:\>` |
| 12 | Abort-on-error + progress instrumentation | capabilities | **in-model, built** (none of the three has this) | `fail_fast`, `echo_steps` |
| 13 | Worked example, stated limits, FAQ accordions on a real page | copy/SEO/UX | **in-model, built** | `page/content.md` |
| 14 | Heading-addressed section extraction (codedown `--section`) | capabilities | **considered, rejected** | overlaps `markdown-section-extractor`; chaining the two beats a ninth param. Heading text already names each step, which covers the discoverability half. |
| 15 | Actually *executing* the blocks, breakpoints, retries, resume-at-step | capabilities | **out-of-model** | needs a real shell/process; this is browser-local wasm. The script we emit is what you run. |
| 16 | Reading `*.md` from disk / a directory, `serve` mode | capabilities | **out-of-model** | no filesystem or server; input is pasted text or a chat/CLI argument. |
| 17 | Secret-aware blocks with prompted values, handlebars templating | capabilities | **out-of-model** | belongs to an executor with a session; there is nothing to interpolate into here. |
| 18 | `<<macro>>` reference expansion between named blocks (Entangled/lmt) | capabilities | **considered, rejected** | that is source-file tangling, a different tool; conflicts with "steps run in document order". |

## Result

Twelve of the fourteen in-model gaps shipped in the first pass; two were rejected on
judgment with the reason recorded above. Three capabilities here have **no** equivalent
in any of the three competitors: prompt-stripping pasted terminal sessions, skip-tagged
blocks surviving as commented-out steps, and the fail-fast/progress instrumentation.

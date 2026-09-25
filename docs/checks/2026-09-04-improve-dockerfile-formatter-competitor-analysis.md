# dockerfile-formatter — competitor scan + build decisions (2026-09-04)

Scan run before implementing, per the create-next-tool recipe. This file paraphrases public
behaviour only; no competitor copy, branding or trademarked language was copied into the tool page.

## Duplicate check

Existing blocks were searched for Dockerfile and container formatting functionality. No committed
block formats Dockerfiles or Containerfiles. General text/code formatters in the repo do not know
Dockerfile instruction keywords, parser directives, `FROM ... AS`, continuation escapes or heredoc
bodies. **Not a duplicate; built.**

## Sources checked

| # | Source | What it showed |
|---|---|---|
| 1 | FastMinify Dockerfile formatter | Browser formatter focused on instruction casing and spacing. |
| 2 | Encode64 Dockerfile repairer | Repair/cleanup language around inconsistent casing, indentation and continuations. |
| 3 | MockerAPI Dockerfile linter/formatter | Formatter plus best-practice linting; uppercase keywords and multi-line RUN indentation are table stakes. |
| 4 | trydevtools Dockerfile formatter | Formatter controls for keyword casing, aligned backslashes and blank lines. |
| 5 | dockerfmt / hadolint references | CLI ecosystem context: formatters normalize layout; linters analyze best-practice rules separately. |

## Table-stakes findings

| Capability | Seen in | Verdict | Where it landed |
|---|---|---|---|
| Paste a full Dockerfile / Containerfile | 1–4 | in-model | Required multiline `input`. |
| Uppercase instruction keywords | 1, 3, 4 | in-model | `instruction_case = upper` default. |
| Lowercase / preserve alternatives | 4 and common team style | in-model | `instruction_case = lower|preserve`. |
| Normalize spacing after instruction keyword | 1, 3 | in-model | First keyword separated from arguments by one space; `FROM ... AS` collapses spacing. |
| Re-indent continuation lines | 2–4 | in-model | `indent` slider, default four spaces. |
| Align trailing continuation backslashes | 4 | in-model | `align_continuations` checkbox. |
| Cap excessive blank lines | 2, 4 | in-model | `max_blank_lines` slider. |
| Separate multi-stage builds | 3, 5 | in-model | `blank_line_between_stages` checkbox. |
| Normalize `#comment` spacing | 1, 2 | in-model | `normalize_comments` checkbox, with banner/directive exceptions. |
| Preserve heredocs and parser directives | Dockerfile grammar requirement | in-model | `# syntax`, `# escape`, `# check` and heredoc bodies are copied through. |
| Lint best-practice rules | 3, 5 | out-of-model for this tool | Listed as out of scope; belongs to a Dockerfile linter, not a formatter. |
| Shell-format RUN bodies with shfmt | dockerfmt | out-of-model here | Would require a shell parser/formatter and can change command semantics; not built. |
| Auto-fix missing continuation characters | 2 | out-of-model here | Ambiguous repair; this tool returns a line-numbered error instead. |

## Decisions

1. Formatting must be conservative: change layout and keyword casing, never reorder instructions or
   rewrite shell/package-manager arguments.
2. Dockerfile-specific safety matters more than generic prettification. Parser directives and
   heredoc bodies are byte-sensitive and therefore preserved.
3. Best-practice linting is intentionally out of scope. The page describes that distinction so users
   do not mistake this formatter for hadolint-style policy checking.
4. The default style mirrors the common ecosystem pattern: uppercase instructions, four-space
   continuations, one blank line between stages, normalized ordinary comments.
5. Controls are explicit for team style differences: casing, indent width, aligned continuations,
   blank-line cap, stage separation and comment spacing are all user-selectable.

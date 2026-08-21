# regex-codemod — competitor analysis (2026-08-21)

Scan run BEFORE implementation. All findings are paraphrased observations of publicly visible
feature sets; no competitor copy, branding, or trademarks are reproduced or reused.

## Scope

`regex-codemod` applies one regex (or literal) find-and-replace across a **pasted multi-file
bundle** — text where several files are concatenated behind header markers (`=== path ===`,
`--- path ---`, `==> path <==`, `# file: path`) — and previews the result as a **unified diff**
per file. That multi-file + diff-preview framing is what separates it from the existing
`blocks/find-replace` (single blob, returns rewritten text only) and from `blocks/text-diff`
(compares two texts, does not transform).

## Competitors reviewed

| Tool | What it does | Notable surface |
| --- | --- | --- |
| QuickTextTools "Regex Find and Replace" | Regex/plain replace over one textarea | Flag checkboxes `g`/`i`/`m`/`s`, diff view, ~20 preset chips, sequential multi-rule mode |
| MiniWebtool "Find and Replace Text" | Regex/plain replace, upload + download | Regex / case-sensitive / whole-word toggles, colour-coded diff, undo history, preset patterns |
| coding.tools "Regex Replace" | JS-regex replace with live preview | Capture groups + backreferences, match count |
| text-case.com "Find & Replace" | Replace with preview | Multi-document mode for batch operations, capture groups |

(Search surfaced several near-identical clones — ToolsKit "Regex Replacer", OneDev Tools,
Just File Tools — which add nothing beyond the four above.)

## Table stakes → decision

| Table stake | Seen at | Decision |
| --- | --- | --- |
| Regex pattern + replacement with `$1`/`${name}` capture references | all four | **In model** — `pattern`, `replacement` params; Rust `regex` expansion syntax |
| Plain-text (literal) mode | QuickTextTools, MiniWebtool | **In model** — `literal` boolean (escapes the pattern, replacement inserted verbatim via `NoExpand`) |
| Case-insensitive flag (`i`) | all four | **In model** — `ignore_case` |
| Multiline flag (`m`) | QuickTextTools | **In model** — `multiline` (`^`/`$` match at line boundaries) |
| Dot-matches-newline flag (`s`) | QuickTextTools | **In model** — `dot_matches_newline` |
| Global vs first-match-only (`g`) | QuickTextTools, coding.tools | **In model** — `replace_all` (default true; false = first match per file) |
| Whole-word matching | MiniWebtool | **In model** — `whole_word` (wraps the pattern in `\b…\b`) |
| Replacement/match count | coding.tools, MiniWebtool | **In model** — `replacements` + `files_changed` counters in the response and in the diff header |
| Diff preview of the change | QuickTextTools, MiniWebtool, text-case | **In model** — `output = "diff"` (default): unified diff with configurable `context` |
| Full rewritten output / download | MiniWebtool | **In model** — `output = "full"`; the page's generic Download link covers the file save |
| Batch / multi-document mode | text-case | **In model, and the core differentiator** — `file_marker` splits the paste into files and every file is processed and diffed independently |
| Preset patterns (emails, HTML tags, dates, camel→snake) | QuickTextTools, MiniWebtool | **In model** — `[[example]]` preset chips on the page (rename-a-symbol, swap date format, strip HTML tags, redact emails) |
| Stated size guidance | QuickTextTools ("split above ~1 MB") | **In model** — hard, documented caps: 1,000,000 input characters and 2,000 files, with an explicit error message |

## Out of model (listed, deliberately not built)

- **Undo/redo history and saved-pattern history** — needs client-side persistent state; gizza
  tools are stateless pure functions. The page's Reset button plus URL deep-links cover re-runs.
- **Sequential multi-rule pipelines** (rule 2 consumes rule 1's output) — would need an array-of-
  rules param and a rule editor UI; a second run of the tool achieves the same result today.
- **Live per-keystroke match highlighting inside the input textarea** — a rich-text editor
  surface, not something a pure compute block plus the generic page runtime can express.
- **Real file uploads / directory batch** — the paste-a-bundle input is the deliberate in-model
  substitute; the page's file-upload control is reserved for the ffmpeg runtime.
- **JavaScript regex dialect specifics** (lookbehind, backreferences inside the pattern) — the
  Rust `regex` crate is deliberately backtracking-free, which is what guarantees linear-time
  matching and no catastrophic-blowup input. Documented on the page as a stated limit, with the
  supported syntax (named groups, Unicode classes) called out.

## UX patterns adopted

- Preset chips (`[[example]]`) instead of a preset dropdown — matches the one-click affordance
  competitors ship, and stays declarative.
- `multiline = true` on the bundle field so pasted newlines survive.
- Friendly `<select>` labels via `[input.labels]` for `file_marker` and `output`.
- Counts surfaced in the output header so the "N replacements in M files" feedback competitors
  show live is present in a single-shot run.

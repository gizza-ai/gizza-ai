# path-extractor — competitor analysis (2026-08-17)

Scan run **before** implementation, so the descriptor could ship the table-stakes
parameters from the start. All findings are **paraphrased**; no competitor copy,
branding, or trademarks were reused.

## Competitors reviewed

| # | Competitor | Shape | Why it counts |
|---|------------|-------|----------------|
| 1 | Facebook PathPicker (`fpp`) | Terminal filter over piped command output | The de-facto tool for "pull the file paths out of this command output" (git status, grep, build logs, stack traces) |
| 2 | TextConverter.io "Files Extractor" | Browser tool, paste text → file list | Closest direct web equivalent: extraction + dedupe + sort + extension filter |
| 3 | miniwebtool List Cleaner | Browser list processor | Sets the bar for post-extraction list handling (dedupe/sort/filter/output shape) |
| 4 | Regular Expressions Cookbook §8.22/8.23 recipes | Canonical regex references | Defines the path forms a serious extractor must recognise (Windows folder vs. filename split) |
| 5 | Splunk `rex` field-extraction threads | Practitioner Q&A | Real-world log/stack-trace path patterns, mixed Windows + POSIX in one stream |

(Texterfly's text extractor was in the search results but its page returns HTTP 403
to automated fetches, so it was replaced by the Regular Expressions Cookbook
recipes as the fifth reference rather than running with four.)

## Table stakes observed → our decision

| Table stake | Seen in | Decision |
|---|---|---|
| Detect POSIX absolute + relative paths (`/usr/lib/libc.so`, `src/main.rs`, `./x`, `../x`) | 1, 2, 5 | **In model** — core matcher |
| Detect Windows drive paths (`C:\Users\me\file.txt`) and UNC (`\\server\share\f`) | 1, 4, 5 | **In model** — core matcher |
| Detect `~/`-relative paths | 1 | **In model** — core matcher |
| Recognise the grep / stack-trace `path:LINE[:COL]` suffix | 1 (highlights the trailing `:LINE`) | **In model** — `keep_line_numbers` param; line/column also surfaced structurally in JSON output |
| Strip surrounding quotes, brackets, parens, trailing prose punctuation | 1, 5 | **In model** — always applied |
| Deduplicate | 1, 2, 3 | **In model** — `dedupe` (on by default), with occurrence counts |
| Sort: keep original order / A→Z / Z→A | 2, 3 | **In model** — `sort` enum |
| Filter by extension, both allow-list and deny-list | 2 | **In model** — `extensions` + `extension_mode` enum |
| Split a path into folder vs. filename | 4 (two separate cookbook recipes), 2 | **In model** — `output` enum (`path`/`filename`/`directory`) |
| Restrict to one path flavour when a log mixes both | 5 | **In model** — `path_style` enum (`any`/`posix`/`windows`) |
| Bare filenames with no directory (`main.rs`) — PathPicker documents this as a known miss | 1 | **In model** and a deliberate differentiator — `require_separator=false` opts into matching extension-bearing bare filenames |
| Output as a plain list, and as a machine-readable form | 2, 3 | **In model** — `format` enum (`list`/`csv`/`json`) |
| Copy + Download buttons on the result | 2 | **Already platform** — the generator gives every `format = "text"` page Copy + Download + Reset |
| Preset / sample-data buttons | 3 (Fruits/Emails/Numbers/Keywords chips) | **In model** — `[[example]]` preset chips (build log, git status, Python traceback) |
| Live word/character count of the input | 2 | **Considered, rejected** — the result header already reports how many paths were found, which is the number that matters here; a character counter is a different tool |

## Out of model (listed, not built)

- **On-disk existence check / "only show files that exist"** (PathPicker's `--no-file-checks`
  inverts this): the tool runs in a sandboxed browser tab or a wasm block with no
  filesystem access, so it cannot stat anything.
- **Interactive selection + "open in $EDITOR" / run a command on the picks** (PathPicker's whole
  UX): needs a terminal UI and process execution.
- **Piping live command output in**: there is no shell attached; input is pasted text or a CLI
  argument.
- **Case transforms, prefix/suffix add-remove, quote-wrapping, shuffle, numbering** (List
  Cleaner): generic list post-processing that belongs to a list-cleaning tool, not a path
  extractor — bundling them would bloat this schema for a different job.
- **Custom output delimiters beyond newline/CSV/JSON**: `list` + `csv` + `json` covers the
  paste-onward cases; arbitrary delimiters are list-formatting, not extraction.

## Notes

- PathPicker's documented limitation ("files that are single words, with no extension, not
  prepended by a directory, will fail to match") is a precision trade-off, not a bug. We keep the
  same high-precision default (`require_separator = true`) but expose the opt-in, and we state
  the trade-off on the page instead of leaving users to discover it.
- Every table stake above landed either in the descriptor or in the out-of-model list; none was
  dropped silently.

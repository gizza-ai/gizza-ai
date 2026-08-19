# keep-to-markdown — competitor analysis (2026-08-06)

Scan run **before** implementation. All notes below are paraphrased observations of what the
tools do; no competitor copy, branding, or trademarked wording is reused anywhere in this repo.

## Tools reviewed

| # | Tool | Shape | Reachable |
| - | ---- | ----- | --------- |
| 1 | `k4j8/google-keep-takeout` (GitHub) | Python script run inside the Takeout folder | yes |
| 2 | `jimmy` (marph91) — `--format google_keep` | General note-app converter CLI, takes the Takeout `.zip`/`.tgz` | yes |
| 3 | `vinid223/gtkoutkeeptomd` (GitHub) | Python CLI, `-i/-o/-r/-a/-f/--archivedoutput` | yes |
| — | `vHanda/google-keep-exporter`, `camjc/keep-to-markdown`, `haydencbarnes/googleKeepExtractor` | skimmed from search result summaries only (same feature family: JSON → Markdown + YAML header, attachments, labels) | partial |

## Observed table-stakes behaviour

| Capability | Seen in | In model here? | Decision |
| ---------- | ------- | -------------- | -------- |
| One Markdown file per Keep note | 1, 2, 3 | yes | Output is a labeled bundle, one `==== name.md ====` section per note (browser has no filesystem). |
| Title → `# heading`, body below | 1, 2, 3 | yes | Implemented. |
| Checklist items → `- [ ]` / `- [x]` task list | 1, 2 | yes | `checkbox_style = task-list` (default), plus `bullet` and `plain`. |
| Labels preserved | 1, 2, 3 | yes | YAML `labels:` list, or `#hashtags` inline. |
| Labels drive folder/subdirectory layout | 1 | yes | `filename_style = label-title` emits `label/title.md` paths in the bundle headers. |
| YAML frontmatter header (title/dates/tags) | vHanda, 2 | yes | `metadata = frontmatter` (default) — title, created, updated, labels, pinned, archived, color. |
| Created / last-edited timestamps | 1, 2, 3 | yes | Keep stores `createdTimestampUsec` / `userEditedTimestampUsec` (microseconds) — converted to ISO‑8601 UTC. |
| Archived notes handled separately | 3 (`-a`, `--archivedoutput`) | partial | `include_archived` toggle + `archived: true` in frontmatter; a separate output *directory* is meaningless for a single text bundle. |
| Trashed notes excluded by default | 1, 3 | yes | `include_trashed`, default off. |
| Attachments preserved | 2, haydencbarnes | partial | The Takeout binaries are separate files we never receive — we emit the Markdown image/file **links** (`link_attachments`, default on) that resolve once you copy the Takeout attachment files next to your notes. |
| HTML export as input | camjc, haydencbarnes | yes | Auto-detected: paste either the per-note `.json`, a JSON array, or the Keep `.html` export. |
| `.zip` / `.tgz` Takeout archive input | 2 | **out of model** | This tool takes pasted text, not an archive upload. Noted on the page. |
| Pinned / color flags | vHanda | yes | Emitted in frontmatter when set (`pinned: true`, `color: BLUE`). |
| Weblink annotations (link chips) | 2 | yes | Appended as a Markdown link list. |
| Writing real files with original file mtimes | 1, 3 | **out of model** | No filesystem in the browser; stated as a limit on the page. |
| Recursive directory conversion | 3 | **out of model** | No filesystem; the JSON-array form covers "many notes at once". |
| Round-trip back into Keep via the unofficial API | keep-it-markdown | **out of model** | Requires network + Google auth; this tool is offline and local-only. |
| PDF output | haydencbarnes | **out of model** | Out of scope for a Markdown converter; `markdown-to-pdf` already exists in this repo. |

## UX controls competitors ship

- Flags for archived separation and recursion (CLI-only ergonomics).
- No competitor ships an interactive page — they are all scripts. The page here adds:
  select controls with human-readable labels for every enum, preset example chips
  (JSON note, HTML export, labels-as-folders), and query-param deep links.

## Worked examples competitors publish

- A shopping-list note with checked/unchecked items rendering as a `- [x]` task list.
- Label name becoming the containing folder.

Both are mirrored (with our own data and wording) in `page/content.md` and the preset chips.

## Gaps closed in this build

Every "yes" row above landed in the descriptor. The out-of-model rows are listed on the page
under limits rather than silently dropped.

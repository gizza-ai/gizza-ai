# file-list-sorter — competitor analysis (2026-08-14)

Scan run before final verification, per the create-tool loop. Query: `online natural sort file list filenames paths by extension size tool`. Notes are paraphrased category observations only; no competitor copy, branding, or trademarks are reused in the tool or page.

## Competitors reviewed

| # | Tool shape | Relevant behavior |
|---|---|---|
| 1 | Browser natural-sort text tools | Paste one item per line, sort with numeric chunks so `file2` comes before `file10`, optional reverse and case-insensitive modes. |
| 2 | Online filename/path sorters | Treat entries as file paths, expose folder/path awareness, basename sorting and duplicate removal for copied `find` / `ls` output. |
| 3 | File-manager sort modes (Explorer/Finder-style behavior) | Natural order, folders first, extension/type grouping, and case-insensitive comparison are expected defaults for user-facing file lists. |
| 4 | CLI pipelines (`sort -V`, `find | sort`, `du -h | sort -h`) | Natural/version sort and size sort are available in shell pipelines, but require knowing flags and preserving size columns. |
| 5 | Spreadsheet/data-cleaning workflows | Users paste file inventories, sort by extension/depth/size, and export numbered or tabular lists for tickets and docs. |

## Table stakes and decisions

| Capability | Decision |
|---|---|
| Natural numeric ordering | Built as the default `sort_by=natural`, so `img2.png` sorts before `img10.png`. |
| Plain alphabetical order | Built as `sort_by=alpha` for machine/codepoint comparisons. |
| Filename-only sort | Built as `sort_by=basename` so folder prefixes can be ignored. |
| Extension/type grouping | Built as `sort_by=extension`, including dotfile and no-extension behavior. |
| Folder-depth sort | Built as `sort_by=depth`, useful for `find` output. |
| Size sorting | Built as `sort_by=size`; recognizes human units before or after the path and requires a real size column. |
| Ascending/descending | Built with `order=asc|desc`; folders-first remains stable in both directions. |
| Case-insensitive default | Built with `ignore_case=true` default, matching file-manager behavior. |
| Folders first | Built with `dirs_first=true` default; folders are inferred from trailing slash or parent entries. |
| Keep folders grouped | Built as `group_by_dir` for sorting inside parent folders. |
| Duplicate removal | Built as `unique`, preserving first spelling. |
| Output modes | Built as `list`, `numbered`, `table`, and `json`. |
| Preset examples | Built with chips for natural order, extension grouping, size sort and depth sort. |

## Out of model / intentionally not built

- Reading the real filesystem or directory picker input: gizza pages operate on pasted text, not local directory handles.
- Moving/renaming files on disk: this tool only sorts the list the user provides.
- Locale-specific collation: deterministic ASCII-ish comparison is preferable for repeatable CLI/page output.
- Recursive folder scanning: use `find`, `git ls-files`, `ls -1` or `du -h` and paste the resulting list.

## Sibling overlap check

Not a duplicate of `sort-lines` (plain line sorting only), `list-converter` (format conversion), `directory-tree-view` (tree rendering), `bulk-file-renamer` (rename planning), or `file-tree-generator` (generates tree output). The distinguishing feature is path-aware file-list sorting with natural, extension, depth, size, folders-first and duplicate controls in one descriptor.

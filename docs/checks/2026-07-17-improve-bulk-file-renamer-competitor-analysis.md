# Competitor analysis — bulk-file-renamer (2026-07-17)

Function: preview bulk filename changes from a list of names using find/replace, regex, sequential numbering, case conversion, prefix/suffix rules, and collision checks.

All findings are paraphrased from public tool pages and common desktop batch-renamer patterns. No competitor copy, branding, or trademarks are reproduced.

## Competitors scanned

1. **Advanced Renamer** — desktop batch rename app with methods for adding text, replacing text, renumbering, case changes, and previewing the resulting names before applying.
2. **Bulk Rename Utility** — desktop utility with many panels for name text, numbering, extension handling, case, regex, and a before/after preview grid.
3. **ReNamer Lite** — rule-based desktop renamer; supports insert/delete/replace, case conversion, serialization, and rule previews.
4. **NameChanger** — simple desktop renamer centered on replace/append/prepend/sequence actions and a preview list.
5. **renameutils / mmv-style command-line tools** — scriptable rename workflows that emphasize pattern matching, regex, dry-runs, and collision avoidance.

## Table-stakes metrics and controls

| capability | in our tool? | notes |
| --- | --- | --- |
| Old → new preview before applying | ✅ | primary output; no file mutation |
| One-name-per-line batch input | ✅ | deterministic and portable across CLI/page/chat |
| Find/replace | ✅ | default mode |
| Regex replacement | ✅ | Rust regex with capture references |
| Sequential numbering | ✅ | `{n}` token with start and padding |
| Preserve or transform extensions | ✅ | checkbox; default preserve |
| Prefix/suffix | ✅ | applied around the generated stem |
| Case conversion | ✅ | lower, upper, title, snake, kebab, camel, pascal |
| Collision warning | ✅ | warns when targets duplicate |
| Rename actual files / ZIP output | ❌ | out-of-model for the generic text form and CLI safety model |
| Drag-and-drop file/folder picker | ❌ | needs browser File System Access or uploaded ZIP bytes |

## UX decisions

- The page uses a multiline textarea for filenames and select boxes for the fixed choices (mode and case style).
- Example chips cover common camera-photo replacement, sequential numbering, and case conversion workflows.
- Numeric controls expose the start number and padding; values outside supported ranges are bounded by the descriptor or normalized by the core.
- The output remains copyable text so users can feed it into scripts or review it before applying changes elsewhere.

## Out-of-model items

- Renaming files on disk, reading folders, or writing a re-zipped archive requires filesystem/archive bytes. This repo's generic page model is field-driven and safe-preview oriented, so the tool intentionally outputs a mapping only.
- Live image/audio/file metadata parsing from uploaded ZIP contents is not included; users paste the names they want to transform.

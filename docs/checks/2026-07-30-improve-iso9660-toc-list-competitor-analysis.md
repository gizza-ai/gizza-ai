# iso9660-toc-list — competitor analysis (2026-07-30)

Tool: **iso9660-toc-list** — lists an ISO 9660 image's volume label, directory tree and file sizes
without extracting payload files. Input is base64 text so the same pure parser works on page, CLI and
chat surfaces.

## Competitor scan

Paraphrased observations from top "view ISO contents" / "ISO file list" tools:

1. Desktop archive managers can open ISO files and browse a tree, often with extract buttons. They
   require local installation and are interactive UI tools rather than single-shot reports.
2. Command-line tools such as `isoinfo`/`iso-info` list volume descriptors and directory records,
   with options for Joliet/Rock Ridge. They are powerful but not browser-local.
3. Online file viewers generally require upload, show a tree, and often focus on extracting files.
   Privacy and size limits vary.
4. General archive inspection tools list ZIP/TAR contents, but usually do not parse ISO 9660 volume
   descriptors or Joliet names.
5. Forensics utilities expose deeper sector-level details, boot catalog data and Rock Ridge metadata,
   beyond what a compact web/CLI helper needs.

## Decisions

| Capability | In model? | Decision |
|---|---|---|
| Volume label | Yes | Built from the primary/supplementary volume descriptor. |
| Directory tree and flat path list | Yes | Built; directories-first sorted output. |
| File sizes without extracting payloads | Yes | Built; reads directory records only. |
| Joliet long filenames | Yes | Built; supplementary descriptor names are preferred. |
| Rock Ridge POSIX mode/symlink metadata | No | Out of scope for this pass; listed as a limit. |
| Interactive tree browsing / extraction | No | Out of current page/chat/CLI model; this is a read-only report. |
| Raw binary upload page | Partially | Current pure text page uses base64 input; binary upload would need a different page control. |

## Verification plan

Use an in-memory minimal ISO fixture for unit and page tests: label `TESTDISC`, root file
`README.TXT` (11 B) and `DOCS/A.TXT` (4 B). Tests assert tree/list/summary output, invalid ISO
errors, and enum drift guard.

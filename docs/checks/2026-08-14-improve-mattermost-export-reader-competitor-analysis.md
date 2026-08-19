# mattermost-export-reader — competitor analysis (2026-08-14)

Scan run before implementation, per the create-next-tool recipe. Notes are paraphrased; no competitor wording, branding or trademarks are reused.

## Sources skimmed

| # | Tool / source | Shape | Reachable |
| --- | --- | --- | --- |
| 1 | Mattermost bulk export / migration documentation | export format reference, not a competitor | yes |
| 2 | Mattermost `mmctl export` and import workflows | command-line export/import pipeline | yes |
| 3 | Community scripts for Mattermost JSONL / Slack-style transcript conversion | local scripts that flatten exports | yes |
| 4 | eDiscovery / chat export viewers for Slack/Teams-style archives | archive-to-readable-review UX patterns | yes |

## What they ship

**Mattermost export/import docs.** The core artifact is a JSON Lines file inside a bulk export archive. Lines are tagged with types such as version, team, channel, user, post, direct_channel and direct_post. The format is designed for migration/import, so the raw file is readable by machines but painful for humans: timestamps are Unix milliseconds, authors and channel labels need metadata resolution, replies and reactions are nested, and attachments are file references rather than inline content.

**Mattermost mmctl workflows.** Operational tools focus on producing or consuming export archives. They are admin/server workflows, not local transcript viewers. They do not turn a pasted `import.jsonl` into a browser-readable audit transcript, and they assume Mattermost access plus server credentials.

**Community conversion scripts.** Existing scripts usually flatten JSONL to CSV/Markdown or convert between chat platforms. Useful patterns: local deterministic parsing, CSV for spreadsheets, and filters by channel/user/date. Common gaps: limited direct-message support, weak display-name resolution, no browser page, and little validation/error guidance for malformed JSONL.

**Chat archive viewers / eDiscovery tools.** Viewer products emphasize filtering, summaries, HTML/CSV exports, and readable threads, but they usually require uploading an archive to a service or installing desktop software. They also cover multiple platforms rather than Mattermost's exact bulk-export schema.

## Table stakes → decisions

| Capability | Seen in | Verdict | Where it landed |
| --- | --- | --- | --- |
| Parse Mattermost JSON Lines by `type` | 1, 2, 3 | in-model | two-pass parser over `version`, `team`, `channel`, `user`, `post`, `direct_channel`, `direct_post`, `emoji` |
| Resolve authors from user metadata | 1, 3, 4 | in-model | nickname → first + last → username fallback |
| Resolve channel display names and privacy | 1, 3 | in-model | channel labels show `#name (Display)` and `[private]` for private channels |
| Render readable transcript | 3, 4 | in-model | text/Markdown/HTML transcript grouped by channel, chronologically sorted |
| Include nested thread replies | 1, 4 | in-model | replies render indented below the root post; checkbox can hide them |
| Include direct messages | 1, 4 | in-model | direct posts become member-labelled sections; checkbox can hide them |
| Aggregate reactions | 1, 4 | in-model | per-message `[reactions: :emoji: xN]` extras |
| Preserve attachment references | 1, 3, 4 | in-model | `[attachment: path]` placeholders; no file unpacking |
| Summary counts | 4 | in-model | version, teams, channels, direct conversations, users, emoji, messages, reactions, attachments, date range, per-channel and per-author counts |
| Filters by channel/user/date | 3, 4 | in-model | `channel`, `user_filter`, `since`, `until` |
| CSV export | 3, 4 | in-model | transcript CSV and stats CSV with quoted cells |
| Max-message preview cap | 4 | in-model | `max_messages`, applied after filters, reported in summary |
| Upload archive / server-side review UI | 4 | out-of-model | gizza tools are local single-page tools with no backend or account |
| Read compressed archive directly | 1, 2 | out-of-model | current page model accepts pasted text, not multi-file archive extraction |
| Fetch attachment binary files | 1 | out-of-model | needs archive filesystem/files; transcript preserves paths only |
| Import into Mattermost or modify server data | 2 | out-of-model | this is a read-only viewer, not an admin migration client |
| Recover deleted/edited message history not in export | — | out-of-model | impossible from a static export file |

## UX patterns adopted

- One paste box with deterministic local parsing, matching the community-script convenience without requiring a terminal.
- Human-readable text as the default, plus Markdown/HTML/CSV for documentation, web pages and spreadsheet review.
- Filters for the audit/eDiscovery pattern: channel, author, date bounds and a preview cap.
- Explicit limits and edge-case notes: paste `import.jsonl`, UTC timestamps, attachment placeholders, no server access and no upload.

# ssh-config-formatter competitor analysis — 2026-08-18

Backlog item: `ssh-config-formatter` — parse, validate, normalize and pretty-print an OpenSSH client config while flagging duplicate hosts, unknown keywords and shadowed Host patterns.

## Sources skimmed

| Competitor | What it exposes | Table-stakes patterns observed | Fit decision |
| --- | --- | --- | --- |
| Simplified Tools — Secure Shell (SSH) Client Config Editor | Paste/edit SSH config source, host filter, grouping by none/wildcard/domain, host-block inventory, generated config, lint, security, directive insights, diff and JSON result tabs. | Paste textarea, summary counters, host inventory, lint/security reports, JSON export, copy/download, grouping/filter controls. | In-model: paste config, format output, lint report, JSON export, host list, duplicate/shadow/wildcard/unknown-keyword checks. Out-of-model for this pure block: interactive host-block editor, draggable reordering, download/copy chrome, security scoring beyond static config lint. |
| SSH Workbench — SSH Config Generator | Visual generator for one or more Host entries with Host alias, HostName, User, Port, IdentityFile, ProxyJump and advanced options; client-side generated config with copy/download. | Default port 22, common directives (HostName, User, Port, IdentityFile, ProxyJump, ForwardAgent, ServerAliveInterval), add-host flow, generated config textbox, reference table. | In-model: recognize/canonicalize common client directives, validate Port, retain ProxyJump/IdentityFile values, produce copyable formatted config. Out-of-model: visual generator/editor for creating hosts from individual form fields. |
| ThisDevTool — SSH Config Generator | Single-host, batch and template tabs; add/remove hosts; global ServerAliveInterval/ServerAliveCountMax; AddKeysToAgent and UseKeychain checkboxes; copy/download generated config; cloud templates. | Multiple host entries, global defaults, checkboxes for yes/no directives, cloud-provider templates, batch/templates mode, quick-reference docs. | In-model: parse multiple Host blocks, preserve global directives before first Host, validate yes/no values, keepalive keyword support, host-list output. Out-of-model: cloud/provider templates and bidirectional form editing. |

## Descriptor decisions

- `text` is a required multiline string with a 10,000-line cap because every competitor starts from either pasted config text or generated host data.
- `output` is an enum: `formatted`, `report`, `json`, `hosts`. This covers the observed generated-config tab, lint/report tab, JSON export, and host inventory.
- `indent` is a bounded integer slider (0–8, default 2) to cover pretty-print spacing without making this a full editor.
- `keyword_case` is an enum (`canonical`, `lower`, `preserve`) because formatters need predictable normalization while still supporting copy-preserving workflows.
- `align_values`, `sort_keywords`, `dedupe`, and `include_notes` are booleans for common formatter/linter toggles and for generating clean copy-paste output.
- `min_severity` is an enum (`info`, `warning`, `error`) to mimic report filtering without implementing a large security-dashboard UI.

## Verification matrix to cover

- Formatted output with canonical keyword spelling and consistent indent.
- Report output that flags wildcard shadowing, duplicate host patterns, unknown/deprecated/server-only keywords and invalid values.
- JSON output with blocks, hosts, issues, stats and formatted text.
- Hosts-only output for inventory use.
- Enum coverage: every `output`, every `keyword_case`, every `min_severity`.
- Non-default checkbox coverage: alignment, sort, dedupe and include-notes disabled.
- Boundary coverage: indent at 0 and 8; line cap rejects more than 10,000 lines.

## Deliberately not built

- No live SSH execution, DNS resolution, `ssh -G`, key-file reading or `Include` expansion; the block only sees pasted text and remains browser-safe.
- No visual host editor, cloud templates, drag-and-drop reordering, copy/download controls or security-score dashboard; those are product UI concerns rather than core model fit for a pure text transformer/linter.
- No server-config (`sshd_config`) validator; server-only keywords are reported as such so users can catch accidental pastes.

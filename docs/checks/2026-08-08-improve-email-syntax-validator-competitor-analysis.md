# email-syntax-validator competitor analysis (2026-08-08)

Backlog row: validate email address syntax and flag disposable or role-based domains against a bundled list.

## Sources scanned

- SmartDomainCheck email tool: advertises syntax, MX, disposable and role-based checks.
- CodenTools email validator: syntax/RFC pattern checks with disposable-domain and role-alias signals.
- ShowMyIP email address validator: syntax, MX records, disposable and role-based flags.
- WebToolsOnline email validator: single/bulk validation with syntax, disposable and role-based detection.
- Autocloz email checker: format, disposable domains, common typos, role/free-provider-style signals.

## Table-stakes and decisions

| Capability / UX pattern | Competitor signal | In current gizza model? | Decision |
| --- | --- | --- | --- |
| Single-address syntax validation | Universal table stake | Yes | Reuse existing `email-validator` core for practical RFC-style syntax and typo warnings. |
| Disposable-domain flag | Commonly advertised | Yes | Reuse existing `disposable-email-detector` bundled offline list and heuristics. |
| Role-based mailbox flag | Advertised by several competitors | Yes | Add local-part role list (`admin`, `info`, `sales`, `support`, `postmaster`, etc.). |
| MX/DNS/domain existence checks | Common in SaaS validators | Out-of-model | Gizza pure blocks do not perform network DNS/SMTP probes; copy states this clearly. |
| Bulk/list validation | Some competitors support bulk | Out-of-model for this row | Existing `email-list-cleaner` handles list workflows; this tool validates one address deeply. |
| Output modes | Competitors provide report or machine-readable results | Yes | Descriptor exposes `report`, `summary`, and `json` enum. |
| Toggle risky checks | Competitor UIs often expose categories | Yes | `check_disposable` and `check_role` booleans, default true. |
| Presets/examples | Common UX pattern | Yes | Page example chips for normal address, disposable role address and syntax-error JSON. |

## Verification focus

- Unit tests cover a valid wrapped address, a disposable role mailbox and invalid syntax/format errors.
- CLI/page tests should assert exact summary output for deterministic cases; detailed reports are stable except typo wording inherited from the existing validator.

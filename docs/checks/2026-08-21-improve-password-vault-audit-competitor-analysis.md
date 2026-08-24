# password-vault-audit — competitor analysis (2026-08-21)

Scan run before implementation. Notes are paraphrased observations of public tool behaviour; no competitor copy, branding, or trademarked text is reused in the page or descriptor.

## Search

Query: "online password vault audit reused passwords weak password manager export".

## Competitors skimmed

### 1. Bitwarden vault health reports
- Checks exposed in paid vault-health views include reused passwords, weak passwords, unsecured websites and inactive 2FA.
- Inputs are native vault entries; output is grouped issue lists rather than a paste-in report.
- Good defaults: prioritize reuse, weak/common passwords, insecure URLs, and missing 2FA separately.

### 2. Dashlane password health
- Summarizes overall password health, weak/reused/compromised categories, and per-login remediation lists.
- Uses a score-style UX and separates reused from weak because a long reused password is still high risk.
- Native app has account context; a local paste tool must rely on export columns.

### 3. NordPass / browser password health checkers
- Common table-stakes: weak passwords, reused passwords, old passwords, and data-breach warnings.
- Breach status typically requires a hosted account or remote check; that is out-of-model for this local pure-WASM tool.
- UX pattern: concise health score plus detailed rows that can be exported or filtered.

## Table stakes extracted

| Capability | Verdict | Where it lands |
| --- | --- | --- |
| Audit many entries at once | in-model | CSV, Bitwarden JSON and line-list readers |
| Reused identical passwords | in-model | `reused-password` findings grouped by fingerprint |
| Duplicate saved logins | in-model | `duplicate-entry` by normalized name/username/url |
| Weak/short passwords | in-model | `weak-password`, `short-password`, score bands |
| Common/breached-password style warning | in-model (offline subset) | bundled common-password list, no network claim |
| Similar variants | in-model | `similar-password` stemming rule |
| Insecure `http://` websites | in-model | `insecure-url` on URL columns |
| Missing 2FA/TOTP field | in-model but optional | `check_missing_2fa`, default false |
| Stale passwords | in-model when dates exist | `stale-password` for export revision dates |
| Overall health score | in-model | 0-100 vault score and weak/fair/medium/strong band |
| Live breach database lookup | out-of-model | would require network/API and handling secrets; explicitly not claimed |
| Native browser/password-manager import | out-of-model | paste/export only in this repo's generic page model |
| Automatic remediation/rotation | out-of-model | requires account-specific integrations |

## Design decisions

1. Passwords are masked by default so audit output is safe to paste into an issue or chat.
2. `format=auto` covers the common paste paths but can be overridden for ambiguous data.
3. `check_missing_2fa` defaults false because many users store second factors outside the password manager.
4. JSON and CSV outputs are included for CI/spreadsheet triage; report output is the default human surface.
5. This is distinct from existing single-password tools: it reasons across many vault entries and reports reuse/duplicates.

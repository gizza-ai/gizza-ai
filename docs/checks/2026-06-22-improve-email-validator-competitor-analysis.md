# email-validator — competitor analysis (2026-06-22)

Syntax-only, browser-private email validator. Three surfaces verified: chat
block (`wafer build` OK, 304 KiB), CLI (`gizza tool email-validator`), page
(`/tools/email-validator/`, 3 Playwright specs green). 21 core/drift unit tests
pass.

## Top competitors surveyed

1. **Hunter Email Verifier** — hunter.io/email-verifier
2. **Verifalia** — verifalia.com/validate-email (30+ step pipeline)
3. **Mailmeteor Email Checker** — mailmeteor.com/email-checker (15+ checks)
4. **Clearout Email Verifier** — clearout.io/email-verifier (14+ checks)
5. **MyEmailVerifier free syntax checker** — myemailverifier.com/free-tool/free-email-syntax-checker

## Capability matrix (in-model = pure, browser-private, no network)

| Capability | In model? | gizza email-validator |
| --- | --- | --- |
| RFC 5321/5322 syntax check (local + domain) | yes | done — atext local-part rules, dot-atom edges, domain labels |
| Missing/duplicated `@` | yes | done (quoted-local tolerated) |
| Length limits (local 64, domain 253, total 254, label 63) | yes | done |
| Leading/trailing/consecutive dots | yes | done (local + domain) |
| Illegal-character detection (incl. spaces, non-ASCII/IDN hint) | yes | done |
| Misspelled-provider "did you mean" (gmial→gmail, etc.) | yes | done + emits corrected address as `Suggestion` |
| Misspelled-TLD detection (.con→.com, .ner→.net, …) | yes | done |
| All-numeric / suspicious TLD warning | yes | done |
| Whitespace / `mailto:` / `Name <addr>` unwrap + trim warning | yes | done |
| Quoted local part + IP-address-literal handling | yes | done (warned, not rejected) |
| Structured valid/errors/warnings/suggestion report | yes | done |
| **DNS / MX record lookup** | NO (network) | out of scope — no network in sandbox |
| **SMTP handshake / mailbox-exists ("deliverable")** | NO (network) | out of scope |
| **Disposable / temp-mailbox detection** | borderline (needs maintained blocklist) | out of scope — would be a stale embedded list |
| **Catch-all / role-account / spam-trap detection** | NO (network/data) | out of scope |
| **Bulk list validation + CSV export** | yes (UI-only) | not built — single-address tool by design |

## Gaps closed this build

All in-model gaps a free syntax checker offers are covered, and the tool goes
beyond the typical free "syntax checker" tier by adding the **did-you-mean
suggestion** (corrected address for both misspelled popular domains and
misspelled TLDs) that competitors gate behind their paid network-verification
products. Errors and warnings are cleanly separated so a well-formed-but-typo'd
address is reported `Valid: yes` with a flagged correction rather than a false
rejection.

## Deliberately out of model (documented, not built)

Every "deliverability" feature the paid competitors lead with — MX/DNS lookup,
SMTP ping, disposable/catch-all/spam-trap detection — requires a network
round-trip to the recipient's mail server (or a constantly-updated remote
blocklist). gizza tools run entirely in the browser / sandbox with no network,
which is the whole privacy proposition, so these are intentionally excluded. The
page copy states plainly that the tool validates *format*, not mailbox
existence, so users aren't misled. Bulk CSV validation is a UI/workflow feature
out of scope for a single-input tool.

No competitor copy, branding, or trademarks were used.

## About this tool

Validate one email address before it enters a signup form, CSV import or allowlist. The tool combines practical RFC 5321/5322 syntax checking (including likely-typo suggestions) with a bundled disposable-domain list, then adds a role-address check for shared local parts such as `admin`, `info`, `sales`, `support`, `postmaster` and `abuse`.

Every run returns a `pass`/`fail` verdict plus a `low`/`medium`/`high` risk grade. Bad syntax or a disposable domain is a `fail` at high risk; a deliverable address that is still worth a human look — a role alias, or a likely domain typo like `ada@gmial.com` — passes at medium risk.

Worked example: `admin@mailinator.com` with both checks enabled returns `fail: admin@mailinator.com (syntax: valid, disposable: yes, role_based: yes, risk: high)` in summary format — the syntax is fine, but the domain is a known throwaway inbox and the local part is a role alias.

Pick `report` for a labelled multiline breakdown, `summary` for a single line, or `json` for machine-readable fields (`valid`, `disposable`, `role_based`, `risk`, `verdict`, `errors`, `warnings`, `suggestion`). Turning a check off reports that category as `not checked` rather than as a clean result.

This is an offline syntax and risk check. It does not perform DNS, MX, SMTP or mailbox-existence probes, so a `pass` verdict means the address is well-formed and not on the bundled risk lists, not that the mailbox can receive mail. It validates one address at a time; use a list cleaner for bulk workflows.

## FAQ

<details>
<summary>Does this contact the domain's mail server?</summary>

No. The tool is intentionally offline and deterministic — the same address always produces the same result. It checks practical syntax rules, a bundled disposable-domain list and optional role-address patterns without any DNS, MX or SMTP network call, so nothing you paste leaves the page.

</details>

<details>
<summary>Why are role-based addresses flagged?</summary>

Addresses like `info@`, `support@`, `admin@` and `postmaster@` usually reach a team or an automated queue rather than one person. They are perfectly deliverable, so they still get a `pass` verdict — but they are graded medium risk because many marketing, onboarding and abuse-prevention workflows want to review or block them. `no_reply@` is matched too: underscores are folded to hyphens before the lookup.

</details>

<details>
<summary>Is the disposable-domain list exhaustive?</summary>

No. Disposable mail providers add domains constantly. The bundled list covers the common throwaway services and their alias domains and subdomains, plus a conservative throwaway-keyword heuristic — it is a fast, high-precision signal, not a registry. A `Disposable: no` result means "not on the known list", so pair it with your own policy for high-risk signups.

</details>

<details>
<summary>What is the difference between the verdict and the risk grade?</summary>

The verdict answers "should this be accepted at all": it is `fail` only when the syntax is invalid or the domain is disposable. The risk grade adds nuance for everything that still passes — `medium` for a role alias or a likely domain typo, `low` for an ordinary personal address. Cosmetic notes, such as the address having had surrounding whitespace that was trimmed, appear under warnings but do not raise the risk grade.

</details>

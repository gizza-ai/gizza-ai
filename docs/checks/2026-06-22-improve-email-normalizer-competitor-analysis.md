# email-normalizer — competitor analysis (2026-06-22)

Tool: `blocks/email-normalizer` — canonicalizes an email address to its
deliverable form (lowercase, strip Gmail dots, drop `+tag` sub-addresses, fold
`googlemail.com` → `gmail.com`) and reports the result.

## Surfaces verified

- **Chat / LLM API** — `descriptor()` single-sources the schema; drift-guard
  unit test passes; `wafer build` validates the chat block (302 KiB).
- **CLI** — `gizza tool email-normalizer email=… [lowercase_local=…]`; verified
  Gmail dot+tag strip, Outlook tag-only strip, case preservation, and the
  invalid-input error path (exit 1).
- **Page** — `/tools/email-normalizer/`; 3 Playwright tests pass (Gmail
  canonicalization, non-Gmail dot preservation, uncheck-to-preserve-case).

## Competitors surveyed

The market is almost entirely **code libraries**, not hosted web tools:

| Competitor | Form | Gmail dots | `+tag` strip | googlemail fold | Providers | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| iDoRecall/email-normalize (Python, PyPI) | library | yes | yes | yes | Google, Microsoft, FastMail, Yahoo | does live MX/DNS lookups to detect provider |
| CorentinTh/email-normalizer (npm) | library | yes | yes | yes | Gmail + configurable | "remove dots, strip plus, domain rename" |
| naile/canonical-emails (.NET) | library | yes | yes | yes | Google, Microsoft, FastMail, Yahoo | static domain rules |
| johno/normalize-email (npm) | library | yes | yes | no | Gmail only | minimal |
| email-normalize (PyPI) | library | yes | yes | yes | provider-specific | async, DNS-backed |

No prominent *hosted, browser-side* normalizer surfaced — this tool fills that
gap (runs locally, no server, no sign-up, plus a CLI and chat surface).

## Gap analysis (fit-to-model)

**Closed / already covered**

- Gmail dot stripping + `googlemail.com` → `gmail.com` fold — done.
- `+tag` sub-address removal for every recognized provider — done.
- Domain + local-part lowercasing (local toggleable) — done.
- Provider coverage matches or exceeds the libraries: Gmail, Outlook/Hotmail/
  Live/MSN, Yahoo, **iCloud**, Fastmail, **Proton** (iCloud + Proton go beyond
  the common Google/Microsoft/FastMail/Yahoo set).
- Syntactic validation with explicit error messages (missing `@`, no domain dot,
  bad label characters) — beyond the minimalist libraries.
- Input cleanup the libraries don't do: unwrap `Name <addr>` display-name and
  `mailto:` prefixes, trim whitespace.
- Reports what changed (stripped tag, dots removed, recognized provider, the
  "cleaned" case/trim-only form) rather than returning just the canonical string.

**Out of model (intentionally not built)**

- **Live MX/DNS provider detection** (iDoRecall, email-normalize): requires a
  network DNS lookup. gizza tools are pure/offline by design, so provider
  detection is by a curated static domain map instead. Listed, not built.
- **Deliverability / SMTP verification**: a network probe, out of scope for a
  pure browser/CLI tool.

## Result

Feature-complete versus the in-model competitor surface; exceeds the common
library set on provider coverage, input unwrapping, validation, and reporting.
The only competitor features not implemented (live DNS provider detection, SMTP
deliverability) are network-bound and out of the pure-compute model.

# disposable-email-detector — competitor analysis (2026-06-23)

Snapshot taken while building the tool. Goal: confirm the in-model feature set is
competitive and capture out-of-model gaps (do **not** build those; no copying of
competitor copy/branding).

## Top competitors (category: "disposable / temporary email checker")

1. **Kickbox "Disposable Email Address Checker"** — single-address form, returns
   true/false "disposable". Marketing front-end for their paid verification API.
2. **Verifalia disposable-email checker** — web form + API; flags disposable and
   also does syntax + MX + mailbox verification (paid tiers).
3. **IsTempMail / Debounce "Free Disposable Email Checker"** — paste an address,
   get disposable yes/no; up-sells a bulk/API verification product.
4. **mailcheck.ai / open-source `disposable-email-domains` list (GitHub)** — a
   community-maintained blocklist (tens of thousands of domains) consumed as a
   library; no UI of its own.
5. **NeverBounce / ZeroBounce disposable check** — disposable flag is one signal
   inside a broader paid deliverability/verification suite.

## Capability diff (✓ have / ✗ out-of-model)

| Capability | Competitors | This tool |
| --- | --- | --- |
| Disposable yes/no for an address | all | ✓ |
| Accept a bare domain (no local part) | some | ✓ |
| Alias-domain coverage (e.g. Guerrilla Mail's sharklasers.com, grr.la) | yes | ✓ |
| Subdomain match (inbox.mailinator.com) | varies | ✓ |
| Keyword/heuristic catch for unlisted throwaways | rare | ✓ (conservative whole-label match) |
| Human-readable reason + matched provider | rare | ✓ |
| 100% client-side / offline / no sign-up | rare (most are paid SaaS) | ✓ |
| Unwrap `Name <addr>` / `mailto:` | rare | ✓ |
| **Live MX / DNS lookup** | Verifalia, ZeroBounce | ✗ out-of-model (block has no DNS; would need a network host call; the tool is intentionally lookup-only) |
| **SMTP mailbox-exists probe** | paid suites | ✗ out-of-model (network + slow + rate-limited) |
| **Exhaustive 50k-domain blocklist** | mailcheck.ai list | ✗ partial — we ship a curated high-precision ~140-domain set, not the full registry (deliberate: fast, offline, low false-positive) |
| **Bulk CSV upload** | paid suites | ✗ out-of-model (page is single-input) |

## Gaps closed during the build

- Broadened the domain list beyond the obvious headliners to include common
  **alias domains** (Guerrilla Mail's `sharklasers.com`/`grr.la`/`spam4.me`,
  Mailinator aliases, YOPmail's `.fr.nf` family, the armyspy/fakename generator
  hosts) so coverage matches what competitors detect for the popular services.
- Added **subdomain** matching and a **bare-domain** input path (several
  competitors only accept a full address).
- Added a conservative **whole-label keyword heuristic** so a brand-new
  `tempmail.*` / `throwaway.*` host is still caught without false-positiving on
  real brands (`contemporary-art.org` is not flagged). Most free checkers do a
  list-only lookup and miss these.
- Output gives a **reason + matched provider**, which most yes/no competitors
  omit; honesty copy states "no" = "not on the known list", a signal not a
  guarantee.

## Out-of-model (intentionally not built)

Live DNS/MX resolution, SMTP mailbox verification, an auto-syncing exhaustive
blocklist, and bulk CSV processing all require network access and/or a different
input surface than the single-field, fully-client-side page model. These are
listed here for completeness and were **not** implemented.

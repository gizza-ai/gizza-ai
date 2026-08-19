# email-phishing-link-scanner — competitor analysis (2026-08-15)

Scan run **before** implementation, per `create-next-tool` step 4. Competitor behaviour is
paraphrased from public product pages; no competitor copy, branding, or trademark is reused.

## Duplicate check (done first)

`ls blocks/ | grep -iE 'phish|link|url|email|mail|safelink|spam|domain'` surfaced 30 candidate
blocks. The four real overlaps were inspected in source, and one was **executed**:

| Existing block | What it actually does | Overlap verdict |
| --- | --- | --- |
| `url-safety-inspect` | Structural phishing heuristics for **one URL at a time** (IP host, `@` userinfo, punycode, lookalike **TLDs**, shorteners, ports, length). No email input, no anchor text, no brand comparison. | Adjacent engine, different input surface. |
| `phishing-header-inspector` | **Headers only** — From/Return-Path/Reply-To/Message-ID/SPF/DKIM/DMARC/Received. Explicitly does not look at the body or its links. | Complementary, no link analysis. |
| `link-extractor` | Extracts `<a href>`/Markdown links + in-page anchors with their visible text. Pure extraction — no risk signal at all. | Provides an input step, not the analysis. |
| `email-spam-score` | Takes a raw email and scores **spamminess**. Its link rules do overlap (`URL_MISMATCH`, `OBFUSCATED_URL`, `LINK_SHORTENER`, `INSECURE_LINK`, punycode). | Closest overlap — measured below. |

`email-spam-score` was run on a four-link phishing sample
(`gizza tool email-spam-score --json '{"email": …}'`). It returned `61/100 (HIGH)` with nine
aggregate rules, of which the link-related ones were `OBFUSCATED_URL`, `URL_MISMATCH`,
`LINK_SHORTENER`, `INSECURE_LINK` — **one line each, for the whole message**. It reported
exactly one anchor mismatch (its `find_anchor_mismatch` returns the first match only), never
named *which* of the four links was the shortener, did not flag the punycode link, and did not
notice that the sender domain `paypa1-secure.com` impersonates `paypal.com`.

So the un-shipped capability is real and twofold:

1. **A per-link report.** Every link listed with its target, its display text, its own score and
   its own findings — not a single message-level score with one representative example.
2. **Brand-lookalike (typosquat / homoglyph / combosquat) matching.** `grep -riE
   'typosquat|levenshtein|skeleton|confusable'` across all 1113 blocks' cores returns only fuzzy
   *string-matching* tools (`fuzzy-name-matcher`, `cluster-similar-values`, …) — nothing compares a
   host against a **brand-domain list**. `url-safety-inspect`'s "lookalike" support is a
   6-entry `LOOKALIKE_TLDS` table (`cm`, `co`, `om`, …); `email-spam-score`'s homoglyph rule only
   detects Latin/non-Latin mixing *inside one word*. Neither can tell that `paypa1-secure.com`
   resembles `paypal.com`.

Decision: **build**, scoped so it does not re-implement its neighbours (it does not re-score
headers, and it does not attempt full SafeLinks/URLDefense decoding — see out-of-model below).

## Competitors reviewed (top 3)

1. **Selzy — Email Link Checker** (`selzy.com/en/free-tools/phishing-link-checker/`)
2. **EasyDMARC — Phishing URL Checker** (`easydmarc.com/tools/phishing-url`)
3. **EmailVeritas — URL Checker** (`emailveritas.com/url-checker`)

Also skimmed for the lookalike-domain half of the problem: CIRCL's typosquatting finder,
WhoisFreaks' and DomainKits' typosquat checkers, PowerDMARC's lookalike-domain checker — all of
which generate permutations (omission, repetition, replacement, transposition, homoglyph,
vowel-swap, wrong-TLD, combosquat) and then resolve them via DNS/WHOIS.

### Table-stakes matrix

| # | Capability | Who ships it | Fit | Where it landed |
| --- | --- | --- | --- | --- |
| 1 | Paste a **whole email**, not just one URL; auto-extract every link | Selzy, EasyDMARC | in-model | `email` param; raw RFC 5322 / HTML body / plain text, auto-detected (`format`) |
| 2 | Per-link **0–100 score with the reasons** listed | Selzy | in-model | Per-link score + severity-tagged findings |
| 3 | Verdict **labels** per link and overall | EasyDMARC (`Good`/`Suspicious`), EmailVeritas (5 labels) | in-model | `MINIMAL/LOW/MEDIUM/HIGH/CRITICAL`, matching the sibling blocks' existing bands |
| 4 | Detect **display text vs href mismatch** | implied by all three (the archetypal email-phish trick) | in-model | `display-target-mismatch` finding, per link, for *every* link |
| 5 | **Lookalike / typosquat / homoglyph** brand domains | CIRCL, WhoisFreaks, PowerDMARC, DomainKits | in-model (offline half) | Skeleton-folding + edit-distance + combosquat + wrong-TLD against a 60-entry built-in brand list plus the user's own `brands` |
| 6 | User supplies **their own protected domains** | PowerDMARC, WhoisFreaks | in-model | `brands` param (tag-list control) |
| 7 | **Shortener** detection ("real destination hidden") | Selzy, EmailVeritas | in-model | `url-shortener` finding |
| 8 | **Redirect-wrapper** awareness ("original URL vs redirected URL") | EasyDMARC, EmailVeritas | partly in-model | `redirect-wrapper` finding + single-level `?url=`-style unwrap, with the unwrapped target scanned too. Full SafeLinks/URLDefense v2/v3 decoding is deliberately left to the dedicated decoder block. |
| 9 | Structural signals: bare-IP host, `@` userinfo, punycode, plain http, odd port, deep subdomains, abused TLDs | all three + CIRCL | in-model | One finding each |
| 10 | A **link cap** with the limit stated | EasyDMARC caps at 20 links | in-model | `max_links` (default 200, 1–1000), plus a stated 1 MiB input cap |
| 11 | Show **only the risky links** on long messages | none (all list everything) | in-model | `only_flagged` checkbox — our addition, cheap and useful |
| 12 | Machine-readable output / API | EasyDMARC (paid API key) | in-model | `report=json`, free, same engine, on CLI + page + chat |
| 13 | **Preset examples** to click | Selzy and EasyDMARC both ship a demo input | in-model | Three `[[example]]` chips (spoofed-brand phish, clean newsletter, shortener + wrapper) |
| 14 | Sender-aware analysis (link domains vs the `From:` domain) | **none** — every competitor takes bare URLs | in-model | The `From:` domain joins the brand set, so a link that *resembles but isn't* the sender is flagged |

### Out-of-model (listed, not built — each needs the network or a model)

Every one of these requires a live lookup or a trained classifier, which contradicts this repo's
browser-local, deterministic, offline model. They are stated as limits on the page, not silently
dropped:

- **Blocklist / threat-feed reputation** (EasyDMARC's known-phishing database, EmailVeritas'
  blacklist status, IPQS' live feeds).
- **Domain age / WHOIS registration date** (EmailVeritas flags domains under 7 days old).
- **SSL certificate inspection** and free-CA heuristics.
- **Live redirect chain following** and shortener expansion (EmailVeritas counts hops; we can only
  say "the destination is hidden").
- **DNS resolution** of generated typosquat permutations — CIRCL/WhoisFreaks confirm which
  lookalike domains are *registered*; we can only say a host resembles a brand.
- **IP geolocation, reverse DNS, bulletproof-nameserver detection.**
- **HTTP fetch, response code, content hash, screenshots, DOM capture** (CheckPhish).
- **ML phishing classifiers** ("over 90% accuracy" claims from Selzy/EasyDMARC). Ours is a
  transparent weighted-rule engine instead — auditable and reproducible, which the ML tools are
  explicitly not.

### UX patterns adopted

- Big paste box as the primary control (all three lead with one) → `multiline = true` on `email`.
- Preset chips instead of a prose "try this" (`[[example]]`), since both link checkers ship a demo.
- Reasons shown inline next to each link rather than behind a details view (Selzy's per-link
  "score + reasons" shape), so the report stays copy-pasteable into a ticket.
- A tag-list control for `brands`, matching how the lookalike checkers take a protected-domain
  list.

### Copy / positioning gaps closed

- The page states plainly that a `MINIMAL` rating is *not* proof a link is safe, and that no
  network call is made — competitors bury this (Selzy's "no network calls / no redirect
  expansion" note is the only comparable disclosure, and it sits below the fold).
- Limits (1 MiB input, 200-link default cap, curated brand/shortener/TLD lists, no live lookups)
  are on the page, not only in code.

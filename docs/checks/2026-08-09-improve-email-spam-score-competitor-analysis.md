# email-spam-score — competitor analysis (2026-08-09)

Scan run BEFORE implementing the block, per `/create-next-tool` step 3. All findings are
**paraphrased** from public marketing/product pages; no competitor copy, wording, branding,
trademark, or trigger-word database was copied. Screenshots were not taken (no login-free
result view without submitting a message).

Search: "free online email spam score checker transparent heuristics link ratio caps trigger words".
Real competitors skimmed (top 3 that actually run a *transparent, rule-based* score, rather than
selling an inbox-placement seat test):

| # | Competitor | Model | Scale | Transparency |
| - | ---------- | ----- | ----- | ------------ |
| 1 | Splitforms Spam Test | client-side, no account | 0–100, higher = spammier | per-rule breakdown, "about a dozen" named heuristics |
| 2 | BetterMerge Spam Checker | server, CAPTCHA-gated | 0–10, higher = spammier | trigger-word table (word · category · impact · count) |
| 3 | MailReach Spam Test | server + seed mailboxes | 0–10, higher = *better* (10 = perfect) | green/red per check, content + DNS/reputation mix |

Also seen but not skimmed in depth (same seed-inbox/DNS/reputation class as MailReach, i.e.
mostly out-of-model for a browser-local tool): MailGenius, Unspam.email, IPQualityScore,
TestMailScore, Fundraise Insider (the last is a pure trigger-word counter that assigns each
matched phrase a 1–4 point impact).

## 1. Splitforms Spam Test

- **Input:** a single plain-text blob (contact-form message, comment, or an inbound email body).
  Live character count; re-scores as you type.
- **Rules (named publicly):** spam phrase lists, ALL-CAPS ratio, repeated punctuation, link
  shorteners, suspicious TLDs, zero-width unicode characters, repeated character runs, multiple
  email addresses, length extremes.
- **Scale/bands:** 0–29 clean · 30–60 suspicious · ~60–100 almost certainly spam.
- **Params/defaults:** none — fixed, non-configurable thresholds.
- **Output:** colour-coded 0–100 score, a verdict label, and a per-rule breakdown listing which
  signals fired and how much each contributed.
- **UX:** four one-click preset examples (clean message / old-school spam / AI-generated spam /
  mixed edge case) plus a Clear button. Emphasises 100% client-side, nothing uploaded.

## 2. BetterMerge Spam Checker

- **Input:** subject line + body, 5,000 characters combined. HTML and `From` handling not
  documented.
- **Checks:** trigger word/phrase matching against a curated database; a second "AI-powered"
  mode that claims to look at context and structure (out-of-model — needs a hosted model).
- **Scale/bands:** 0–10. 0–3 excellent · 4–6 moderate, may need optimisation · 7+ high spam
  probability.
- **Output:** score, word + character counts, a safe/risky gauge, a detailed table with one row
  per matched trigger (**word/phrase · category · impact level · count**), and remediation
  suggestions.
- **UX:** live character counter, gauge, per-trigger table, actionable recommendations.
- **Friction:** CAPTCHA before every run; server round-trip.

## 3. MailReach Spam Test

- **Content checks:** spam words, insecure or broken links, **image-to-text ratio**, HTML
  complexity, attachments, tracking pixels.
- **Infra checks:** SPF, DKIM, DMARC, MX, reverse DNS, domain age, domain + IP blacklists,
  seed-inbox placement across 30+ mailboxes.
- **Scale:** 0–10 where 10 is perfect deliverability (inverted vs. the other two); explicitly
  notes a perfect score isn't always attainable.
- **Output:** green/red indicator per check plus tailored recommendations.
- **Options:** address-list separators, manual vs. scheduled runs, report export, Slack/webhook
  notifications.

## Table stakes → what we ship

Every table-stake below is either **in the descriptor** or explicitly listed as out-of-model —
nothing dropped silently.

### In-model (built into `email-spam-score`)

| Table stake | Seen at | How we cover it |
| ----------- | ------- | --------------- |
| Named, weighted trigger phrases with a category | 1, 2, Fundraise Insider | ~100 phrases across 6 categories (urgency, money, marketing hype, credentials/phishing, pharma/health, gambling/adult), each 1–4 points, reported with category + count |
| ALL-CAPS ratio | 1 | `CAPS_RATIO` on body, `SUBJ_CAPS` on subject; ratio reported as a stat |
| Repeated punctuation | 1 | `EXCESS_PUNCT` (runs of `!`/`?`), `SUBJ_EXCLAIM` |
| Repeated character runs | 1 | `REPEATED_CHARS` (e.g. `FREEEEE`) |
| Link shorteners | 1, 3 | `LINK_SHORTENER` over a known shortener-host list |
| Suspicious TLDs | 1 | `SUSPICIOUS_TLD` |
| Zero-width / unicode tricks | 1 | `ZERO_WIDTH`, `MIXED_SCRIPT` (Cyrillic homoglyphs inside Latin words) |
| Multiple email addresses in body | 1 | `MANY_ADDRESSES` |
| Length extremes | 1 | `VERY_SHORT_BODY`, `VERY_LONG_BODY` |
| Link ratio / density | brief, 3 | `LINK_DENSITY` (links per 100 words) + `MANY_LINKS`; density reported as a stat |
| Image-to-text ratio | 3 | `IMAGE_HEAVY` on HTML input; image count + ratio reported |
| Insecure links | 3 | `INSECURE_LINK` (plain `http://`) |
| Tracking pixels | 3 | `TRACKING_PIXEL` (1×1 / zero-size `<img>`) |
| Hidden text | (common filter rule) | `HIDDEN_TEXT` (`display:none`, `font-size:0`, white-on-white) |
| Anchor-text vs. href mismatch | (common filter rule) | `URL_MISMATCH` |
| Obfuscated URLs | new | `OBFUSCATED_URL` (userinfo `@`, punycode `xn--`, bare-IP host) |
| SPF/DKIM/DMARC **results already stamped in the headers** | 3 | `AUTH_FAIL` / `AUTH_MISSING`, and a score-*reducing* `AUTH_PASS` |
| Header anomalies | 3 | `FROM_RETURNPATH_MISMATCH`, `REPLYTO_MISMATCH`, `DISPLAY_NAME_SPOOF`, `MISSING_MESSAGE_ID`, `MSGID_DOMAIN_MISMATCH`, `MISSING_DATE`, `NO_RECEIVED`, `UNDISCLOSED_RECIPIENTS`, `PRECEDENCE_BULK` |
| Score-reducing / positive signals | 3 (green lights) | `AUTH_PASS`, `HAS_UNSUBSCRIBE` (List-Unsubscribe header or an unsubscribe link) |
| Per-rule breakdown with points | 1, 2 | `report=detailed` lists every fired rule with `+N`/`-N`, id, and a plain-English reason |
| Counts/stats block | 2 | words, caps ratio, links, unique link domains, link density, images, trigger hits, punctuation runs |
| Explicit bands | 1, 2 | 0–29 LOW · 30–59 MEDIUM · 60–79 HIGH · 80–100 CRITICAL, printed with the score |
| One-click preset examples | 1 | three `[[example]]` chips (clean newsletter / classic spam / spoofed-header phish) |
| Machine-readable output | (none of the three) | `report=json` — our addition, for scripting/CI |
| No account, no CAPTCHA, nothing uploaded | 1 (only) | browser-local wasm + `gizza` CLI |

**Defaults chosen:** `format=auto` (sniff raw-headers vs. HTML vs. plain text), `report=detailed`,
`check_headers=true`. Score is 0–100, higher = spammier (matches competitor 1; competitor 3's
inverted 0–10 was rejected as confusing next to competitor 1/2's higher-is-worse convention).
The 0–100 scale was preferred over 0–10 because a per-rule breakdown with integer points needs
more resolution than 10 buckets.

### Out-of-model (considered, NOT built)

Everything here needs a server, DNS, or a hosted model, which a browser-local wasm tool cannot do:

- **Seed-inbox placement testing** across real mailboxes (3) — needs mail infrastructure.
- **Live SPF/DKIM/DMARC/MX/rDNS record lookups** (3) — needs DNS. We read the
  `Authentication-Results` the receiving gateway already stamped instead, and say so on the page.
- **Domain/IP blacklist (RBL) checks, sender reputation, domain age** (3) — needs network lookups.
- **Actual SpamAssassin scoring** — needs the rule corpus, Bayes DB, network tests (RBL/Razor/DCC).
  We are explicitly a transparent *heuristic* approximation, not a SpamAssassin verdict.
- **"AI-powered" contextual analysis** (2) — needs a hosted LLM; gizza blocks are pure Rust.
- **Attachment scanning / AV** (3) — no attachment bytes in a pasted message.
- **Broken-link checking** (3) — needs HTTP fetches. We flag insecure and obfuscated links only.
- **Scheduled runs, exports, Slack/webhook notifications** (3) — account/backend features.

### Considered, rejected

- **Adjustable per-rule weights / sensitivity slider.** Rejected: it makes scores
  non-comparable between runs and bloats the schema. The weights are published on the page
  instead, so the score stays reproducible and auditable.
- **Live re-score on every keystroke** (1). Rejected: the shared page driver already runs on
  input change; a bespoke debounce would be per-tool JS, which the workspace rules forbid.
- **CAPTCHA / character cap of 5,000** (2). Rejected outright — we cap at 1 MB purely to keep
  the wasm run bounded, and state the cap on the page.

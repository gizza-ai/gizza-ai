# url-safety-inspect — competitor analysis (2026-07-13)

Scan of the leading online "is this URL safe / phishing link checker" tools, to fix the
table-stakes for a **pure, offline, structural** URL inspector and decide what is in-model vs
out-of-model for gizza (pure-Rust WASM, no network, no ML models).

## Competitors scanned (top 3 real tools)

1. **IPQS Malicious URL Scanner** (ipqualityscore.com) — paste a URL, get a risk score plus
   flags. Combines ML classification, redirect/cloaking tracking, live domain reputation, and
   forensic content analysis. Heavily network + model driven.
2. **PowerDMARC Phishing Link Checker** (powerdmarc.com) — paste a full URL or domain, click
   *Check URL*; returns a trust score, results from two independent threat databases, **and a
   structural heuristic breakdown** of signals, no signup. Closest in shape to a paste-and-score
   heuristic read.
3. **urlscan.io** — submits the URL, actually loads the page in a sandbox, and reports
   screenshots, resolved redirects, TLS certificates, contacted domains, and blocklist hits. Fully
   live/network-based scanner.

(Also noted, not deep-read: NordVPN Link Checker, Bitdefender Link Checker, URLVoid, CheckPhish —
all rely on live blocklists / page loads / brand-monitoring feeds.)

## Table-stakes (paraphrased — no copy reused)

| Capability | In gizza's model? | Decision |
|---|---|---|
| Paste a single URL and get an at-a-glance risk verdict | Yes | **In** — MINIMAL/LOW/MEDIUM/HIGH/CRITICAL rating |
| A numeric score, not just a word | Yes | **In** — 0–100 composite score |
| A breakdown of *why* (individual signals) | Yes | **In** — per-finding list with severity + explanation |
| Detect IP-literal hosts | Yes (structural) | **In** — `ip-literal-host` |
| Detect deceptive `@`/userinfo in the authority | Yes (structural) | **In** — `userinfo-at-sign` |
| Flag punycode / homograph domains | Yes (structural) | **In** — `punycode-label` |
| Flag suspicious / lookalike TLDs | Yes (static list) | **In** — `suspicious-tld`, `lookalike-tld` |
| Flag excessive subdomains / hyphen-stacked / long URLs | Yes (structural) | **In** — `excessive-subdomains`, `hyphenated-host`, `excessive-length` |
| Flag plain http / non-standard port | Yes (structural) | **In** — `no-https`, `non-standard-port` |
| Note URL-shortener hosts | Yes (static list) | **In** — `url-shortener` (info) |
| Credential/urgency keyword detection | Yes | **In** — `deceptive-keywords` |
| Runs without signup, instant | Yes | **In** — local WASM, no network |
| Deterministic / reproducible verdict | Yes | **In** — differentiator vs ML tools |

## Out-of-model (listed, not built)

These all require live network access, a hosted blocklist/reputation feed, or an ML model — none
of which a pure offline WASM block can provide:

- **Live blocklist / threat-database lookups** (Google Safe Browsing, PhishTank, 30+ engines).
- **Expanding/resolving shorteners and following redirects** to the real destination.
- **Loading the page**: screenshots, DOM tree, contacted domains, injected-script analysis.
- **TLS certificate inspection** (issuer, age, mismatch).
- **Domain reputation / age / WHOIS / hosting** signals.
- **ML phishing classification** and brand-impersonation monitoring.

gizza's tool is deliberately the **offline, private, deterministic structural** half of these
tools: no request is ever made to the inspected site, so it is safe to run on a live suspicious
link and gives the same verdict every time. A clean rating means "no structural red flags", not a
safety guarantee — stated in the page limits and FAQ.

Sources: [IPQS](https://www.ipqualityscore.com/threat-feeds/malicious-url-scanner),
[PowerDMARC](https://powerdmarc.com/phishing-link-checker/), [urlscan.io](https://urlscan.io/).

# phishing-header-inspector competitor analysis (2026-07-13)

## Sources scanned

Search query: `email header analyzer phishing spoofing Return-Path From mismatch Received headers SPF DKIM DMARC tool`.

Reviewed real tools/pages from the search results:

1. Ciphers Security email header analyzer — positions itself around tracing the Received chain, SPF/DKIM/DMARC checks, and common spoofing patterns.
2. PowerDMARC phishing email checker — emphasizes visible From vs true sender, authentication results, sender IP/routing clues, and phishing triage.
3. TechClick mail header analyzer — lists SPF, DKIM, DMARC, ARC, From-vs-Return-Path, Reply-To redirection, slow hops, and live DNS lookups.
4. EvilMail email header analyzer — emphasizes local/private analysis of raw headers, travel path, SPF/DKIM/DMARC, and actual sender details.
5. DNSTrack email header analyzer — focuses on parsing headers, tracing the route, and checking SPF/DKIM/DMARC authentication.

No competitor copy, branding, or UI text was reused; the list above is paraphrased feature analysis.

## Table-stakes capabilities and fit

| Competitor pattern | In gizza model? | Decision |
| --- | --- | --- |
| Paste a raw email header block | Yes | Built as required `headers` multiline input. |
| Parse folded RFC 5322 header lines | Yes | Built in core parser; folded continuation lines are joined. |
| Show From, Return-Path, Reply-To, and sender domain relationships | Yes | Built into the report and risk findings. |
| Flag visible/display-name spoofing | Yes | Built for display names that contain an email/domain different from the actual From mailbox. |
| Summarize SPF, DKIM, and DMARC pass/fail/none/neutral values from Authentication-Results | Yes | Built from `Authentication-Results` and `Received-SPF` headers. |
| Trace or count Received hops | Yes | Built as Received hop count plus warnings for missing, short, or private/internal-looking hops. |
| Explain Message-ID sender-domain mismatch | Yes | Built as a low-severity context finding. |
| Risk score / severity summary | Yes | Built as deterministic MINIMAL/LOW/MEDIUM/HIGH/CRITICAL plus 0-100 score. |
| Full route geolocation, sender IP reputation, blacklist checks | No — requires network/reputation service | Listed as out-of-model; not built. |
| Live SPF/DMARC DNS record lookup | No — requires DNS/network | Listed as out-of-model; not built; page explains this limitation. |
| ARC validation and cryptographic DKIM signature verification | Partly out-of-model for this pure offline parser | Not built; parser reads stamped DKIM results but does not verify signatures or ARC chains. |
| File upload of .eml | Mostly in-model but broader than current text-paste tool | Deferred; text paste covers the table-stakes workflow without adding file parsing complexity. |

## UX controls and defaults

- Primary input is a large multiline text area with a realistic header placeholder.
- `report_mode` is an enum (`detailed` default, `summary` optional) so the page renders a select rather than a free-text box.
- `check_received` is a default-on checkbox; users can disable it for intentionally truncated header snippets.
- Example chips cover an aligned sender and a spoofing-indicator sample.
- The page states privacy, supported inputs, and the no-network/no-DNS limitation.

## Worked examples used for implementation

- Aligned sender: From and Return-Path share `example.com`, authentication results are pass, and two Received hops are present. Expected minimal risk.
- Spoofing sample: display name contains `alerts@paypal.com`, actual From uses `evil.example`, Return-Path and Reply-To use other domains, SPF and DMARC fail, DKIM is none, and the Received path includes an internal IP. Expected high/critical risk with multiple findings.

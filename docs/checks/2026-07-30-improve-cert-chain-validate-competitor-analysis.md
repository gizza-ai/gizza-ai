# Competitor analysis: cert-chain-validate (2026-07-30)

## Scanned tools

- SSLShopper-style certificate checker pages: focus on pasted PEM or host-derived chain summaries, expiry dates, issuer/subject display and chain order diagnostics.
- decoder.link / SSL tools style certificate decoders: expose parsed certificate fields, serial numbers, SANs, validity windows and issuer relationships.
- OpenSSL command-line workflows (`openssl verify`, `openssl x509 -text`): table-stakes offline verification of a leaf against supplied intermediates/roots, with detailed failure messages.

## Table-stakes capabilities

| Capability | Decision | Notes |
| --- | --- | --- |
| Paste PEM certificate or bundle | In model | Single multiline field accepts one or more `CERTIFICATE` PEM blocks. |
| Leaf-to-root chain order checking | In model | Child issuer must match next certificate subject. |
| Signature linkage | In model | Each child signature is verified with the next certificate public key; self-signed root is verified when present. |
| Expiry / not-yet-valid checks | In model | Every certificate must be valid at the current time. |
| Subject, issuer, serial and validity display | In model | Report prints the main debugging fields per certificate. |
| CA flag check on issuers | In model | Intermediates/roots used as issuers must have `basicConstraints CA:true`. |
| Browser/OS trust-store verdict | Out of model | Requires platform-specific root stores and policies. |
| Hostname/SAN match | Out of model for this first tool | Current backlog item describes pasted chain validation, not a host or expected DNS name parameter. |
| Revocation / OCSP / CRL / CT logs | Out of model | Requires network access and policy decisions. |
| AIA fetching / incomplete-chain repair | Out of model | Requires HTTP fetching and trust policy. |

## UX decisions

- Use one large textarea because competing tools center on PEM paste workflows.
- Keep the output as plain text for CLI/page parity and easy copying into tickets.
- The report starts with an unmistakable `Certificate chain: VALID` line, then explains exactly what was and was not checked.
- Failure messages point to certificate positions (`#1`, `#2`) so users can reorder or replace the offending certificate.

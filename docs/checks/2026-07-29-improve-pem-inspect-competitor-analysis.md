# pem-inspect — competitor analysis (2026-07-29)

Tool function: parse and pretty-print PEM-encoded X.509 certificates, CSRs, and public/private
keys — subject, issuer, validity, SANs, algorithm, fingerprints. All processing is browser-local
(wasm), no upload. Research is paraphrased; no competitor copy/branding is reproduced.

## Competitors surveyed

1. **certdecoder.com** — client-side X.509 PEM certificate decoder.
2. **certificatedecoder.dev** — X.509 decoder; supports certificate chains / multiple certs and
   drag-and-drop file upload.
3. **diffcheck.org PEM decoder** — X.509 PEM decoder with a "Load Sample" button.
4. **certdecoder.com / aquilax / io-tools (aggregate)** — corroborated field lists from the
   search-result summaries.

## Table-stakes fields (what every decoder shows)

| Field | Competitors | In our model? |
| --- | --- | --- |
| Version (v1/v2/v3) | dev, diffcheck | yes — `version` |
| Serial number | all | yes — `serial` (hex) |
| Subject DN | all | yes — `subject` |
| Issuer DN | all | yes — `issuer` |
| Validity Not Before / Not After | all | yes — `not_before` / `not_after` |
| Expiry status / days remaining | several | yes — `status` + `days_until_expiry` (in-model: page passes current time) |
| Subject Alternative Names (DNS/IP) | all | yes — `subject_alt_names` |
| Signature algorithm | all | yes — `signature_algorithm` |
| Public key algorithm + size/curve | all | yes — `public_key` (algorithm + size/curve) |
| Basic constraints (CA flag) | dev, diffcheck | yes — `is_ca` |
| Key usage / extended key usage | certdecoder, dev | yes — `key_usage` / `extended_key_usage` |
| SHA-256 / SHA-1 fingerprint (thumbprint) | certdecoder, dev | yes — `fingerprint_sha256` / `fingerprint_sha1` |
| Multiple certs / chain in one paste | certificatedecoder.dev | yes — parses every PEM block in the input |
| Load-sample / worked example | diffcheck, dev | yes — page `[[example]]` chip + worked example in copy |
| 100% client-side (no upload) | all | yes — wasm, nothing leaves the browser |

## Our differentiators (beyond the cert-only competitors)

- **CSR decoding** (`CERTIFICATE REQUEST`): subject, public-key algorithm/size, signature
  algorithm, requested SANs. None of the three competitors decode CSRs.
- **Standalone public keys** (`PUBLIC KEY` SPKI, `RSA PUBLIC KEY` PKCS#1): algorithm + size/curve.
- **Standalone private keys** (`PRIVATE KEY` PKCS#8, `RSA PRIVATE KEY`, `EC PRIVATE KEY`): reports
  the key *type/algorithm/size only* — never prints secret scalars.
- **Structured JSON output** — machine-readable, good for the chat/CLI surfaces and scripting.

## UX controls competitors ship (and our decision)

- Paste textarea + decode button → in-model (page provides multiline field + auto-run).
- "Load Sample" / "Show Sample" button → in-model via a `[[example]]` preset chip.
- Copy / Clear result → in-model (generator gives Copy + Reset automatically).

## Out-of-model (considered, not built)

- **Drag-and-drop file upload** of a `.pem`/`.crt` file — the generator's file input is for the
  ffmpeg media family; pure text tools take a paste field. A user pastes the PEM text instead.
- **Fetch-a-live-host's certificate** by hostname (server connect) — needs network/TLS I/O, not
  the browser-local wasm model.
- **Social sharing / language selection** (certificatedecoder.dev) — not a decoding capability.
- **Certificate-chain trust validation** (verifying each cert signs the next, revocation/OCSP) —
  needs trust-store + network; out of the local, single-paste model. We decode each block
  independently.

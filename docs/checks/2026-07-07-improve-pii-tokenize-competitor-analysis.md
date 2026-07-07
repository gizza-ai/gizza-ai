# pii-tokenize — competitor analysis (2026-07-07)

Tool function: replace detected PII in text with **deterministic, format-preserving
pseudonyms** so the text stays linkable (same input → same token) but de-identified.
Distinct from `blocks/redact-pii`, which only masks (`***`) or labels (`[EMAIL]`) —
those destroy linkability. This tool keeps referential integrity.

## Competitors scanned (paraphrased — no copy/branding reproduced)

1. **Google Cloud Sensitive Data Protection (DLP)** — offers deterministic encryption
   (AES-SIV with a surrogate annotation), format-preserving encryption (FPE-FFX, token
   keeps the input's length + character set), and crypto-hash pseudonymization. Detects
   many infoTypes (email, phone, SSN, credit card, IP). Reversible with the stored key.
2. **IRI DarkShield / FieldShield** — reversible and irreversible PII pseudonymization
   across structured + unstructured sources; FPE for well-defined alphabets (SSN, card).
3. **General tokenization vendors (K2view, Protecto, Basis Theory-style)** — the common
   table-stake framing: deterministic tokens produce the same token for the same input
   within a scope, preserving referential integrity for joins/dedup; a key/secret scopes
   the mapping; reversible detokenization needs a vault or a reversible cipher.

## Table stakes (each tagged in-model / out-of-model)

| Capability | Decision |
|---|---|
| Detect email / phone / SSN / credit-card / IPv4 / IPv6 | **in-model** — regex detection (same categories as redact-pii) |
| Deterministic: same value → same token | **in-model** — keyed HMAC-SHA256 keystream |
| Format-preserving: token keeps length + char class | **in-model** — per-character class substitution (digit→digit, a-z→a-z, A-Z→A-Z, punctuation kept) |
| Referential integrity / linkable across occurrences | **in-model** — follows from determinism |
| Keyed / secret so the mapping is stable + unique per key | **in-model** — `secret` param (HMAC key); blank uses a fixed built-in seed |
| Card tokens stay Luhn-valid (pass format validators) | **in-model** — Luhn check digit recomputed on the pseudo card |
| Keep the email domain (segment by provider) vs pseudonymize it | **in-model** — `preserve_email_domain` boolean toggle |
| Reversible detokenization / re-identification via a vault | **out-of-model** — a stateless one-way keyed hash cannot be reversed; true reversal needs a stored token vault or a reversible FPE (FF1/FF3) cipher + persistent key. Spiked the `fpe` crate: FF1/FF3-FFX is feasible to compile but requires a persistent key store to detokenize, out of scope for a single-shot stateless browser tool. Listed, not built. |
| NER for names / street addresses / dates of birth | **out-of-model** — needs a trained model; gizza is pure-Rust + ffmpeg (same limit as redact-pii) |
| Type-prefixed surrogate tokens (`EMAIL_…`) | **not built by design** — that is the labeling mode, which `redact-pii` already covers and which breaks format-preservation |
| Policy engine / per-field RBAC / audit logs | **out-of-model** — server/enterprise concern, N/A for a local browser tool |

## UX controls competitors ship → ours

- Key/secret input → `secret` text field.
- Format-preserving toggle → always on (it is the tool's whole point); domain-keep is the
  one meaningful toggle → `preserve_email_domain` checkbox.
- Worked examples / presets → one `[[example]]` preset chip with sample PII text.

## Out-of-model list (stated on the page, NOT built)

- Reversible detokenization / vault-based re-identification.
- Name / street-address / DOB detection (free-text NER).
- Per-field policy engine, RBAC, audit logging.

Sources (paraphrased, not quoted): Google Cloud DLP pseudonymization docs, IRI
pseudonymization overview, and general tokenization vendor explainers (K2view, Protecto).
</content>
</invoke>

# csr-generator competitor analysis (2026-08-23)

Tool: `csr-generator` — generate a fresh private key and PKCS#10 Certificate Signing Request locally.

## Competitor scan

Search query: `online CSR generator certificate signing request generator SAN DNS IP email options`.

1. `csrgenerator.com` — table-stakes form flow: generate a new private key and CSR together; ask for common name/domain and organization/location subject fields; return PEM blocks suitable for CA submission.
2. `simplified.tools/generate_csr` — table-stakes controls: RSA key generation, SAN and key-usage controls, downloadable PEM output, status/progress copy while key material is generated.
3. `showdns.net/csr-generator` — table-stakes controls: primary domain, organization details, multiple domains/SANs, algorithm choice with RSA for broad compatibility and ECDSA for smaller/faster keys.

## In-model decisions implemented

- Fresh key generation and CSR creation happen locally in the block using pure Rust crates.
- Subject fields: `common_name` (required), `organization`, `organizational_unit`, `country`, `state`, and `locality`.
- Subject Alternative Names: separate multiline/comma/semicolon fields for DNS, IP, email, and URI SANs; optional `DNS:`, `IP:`, `email:`, and `URI:` prefixes accepted.
- Algorithm choice: `p256` default and `p384` enum options. Both produce PKCS#8 private key PEM, public key PEM, and CSR PEM.
- Safety/default behavior: if the user supplies a DNS-like common name and no SANs, the common name is mirrored into DNS SAN because modern public CAs validate SANs rather than CN-only CSRs.
- Validation: required CN, two-letter country codes, parseable IP SANs, IA5/ASCII SAN values, and self-parse sanity check for generated CSR.

## Out-of-model or intentionally deferred

- RSA CSR generation: common on competitor tools, but this first wasm-safe implementation uses RustCrypto ECDSA keys and a compact PKCS#10 encoder. Adding RSA would require a separate RSA signing path and larger validation matrix; documented as out-of-model for this pass.
- Browser page/download UX: existing nondeterministic key-generation tools in this repo are chat + CLI only because live-recompute pages would regenerate secrets on every input change. This tool follows that pattern.
- Extended key usage/key usage request controls: useful for some internal CAs, but not required by all public CA CSR flows and can be added later if the descriptor/page model grows a stable UX for advanced CSR extensions.

## Verification focus

- Core tests prove generated CSR PEM parses and carries expected output structure, including SAN summary and p384 coverage.
- CLI exact-output checks should assert PEM headers and JSON fields rather than deterministic key bytes, because the key/CSR are intentionally fresh on every run.

# pdf-signature-inspector — competitor analysis (2026-06-22)

## What the tool does
Extracts the embedded digital signatures from a PDF and reports, per signature:
the **signer** (certificate subject + issuer Distinguished Names, taken from the
SignerInfo's certificate, not the chain root), the **signing time** (the
cryptographically-signed CMS `signingTime` attribute, plus the signature
dictionary `/M` date as a fallback), the signature **handler / sub-filter**
(`Adobe.PPKLite`, `adbe.pkcs7.detached`, `ETSI.CAdES.detached`, …), the
human-supplied **name / reason / location**, and whether the **`/ByteRange`
digest window is structurally intact** (covers the whole file except the
`/Contents` gap, so nothing was appended/altered outside the signed region).

Pure-Rust: `lopdf` (PDF object model) + RustCrypto `cms`/`x509-cert`/`der`
(PKCS#7 SignedData parse). Surfaces: **chat + CLI** (no page — binary file input
→ structured JSON, the same "no-page file-input" shape as `pdf-extract-text`).

## Top competitors

1. **PDF SignCheck** (pdfsigncheck.com) — cryptographically validates PKCS#7/CMS,
   independently recomputes the document hash over the byte ranges and compares
   it to the signed hash, returns a stamped "Verified" PDF. Free, no sign-up.
2. **eSign.AI Signature Verification** — verifies PAdES / X.509, detects
   post-signing modification, runs locally in-browser (files never uploaded).
3. **DevToolCafe PDF Signature Checker** — extracts signer info, validates
   certificate expiration + chain, checks integrity, 100% browser-based.
4. **iReadPDF Validate Signature** — examines the cryptographic signature,
   validates the certificate chain, confirms document integrity.
5. **Adobe Acrobat / iLovePDF validate-signature** — full PAdES validation with
   trusted-root chain building and revocation (OCSP/CRL) checks.

## Capability diff & gap ranking (fit-to-model)

| Capability | Competitors | gizza tool | Verdict |
|---|---|---|---|
| Extract signer subject/issuer DN | yes | **yes** (signer cert matched by IssuerAndSerialNumber, not first/root cert) | parity |
| Signing time | yes | **yes** (signed CMS `signingTime` + `/M` fallback) | parity |
| Sub-filter / handler reporting | partial | **yes** | parity / slight edge |
| Multiple signatures per doc | yes | **yes** (walks AcroForm `/Fields` + `/Kids`) | parity |
| Reason / location / name fields | partial | **yes** | parity |
| `/ByteRange` structural integrity (no appended bytes) | yes | **yes** (well-formed + reaches EOF check, with a human note on trailing-byte tampering) | parity |
| Honest scope labelling | rare | **yes** (`integrity_check` field states it's structural, not a full crypto verification) | edge |
| **Cryptographic digest recompute & compare** (hash byte ranges vs signed hash) | yes (SignCheck, eSign.AI) | **no** | out-of-model gap (see below) |
| **Certificate-chain trust / root validation** | yes (Acrobat, DevToolCafe) | **no** | out-of-model gap |
| **Revocation (OCSP/CRL) + cert expiry checks** | yes (Acrobat) | **no** | out-of-model gap |
| **Timestamp-token (RFC 3161 / PAdES-LTV) validation** | partial | **no** | out-of-model gap |

## Gaps closed this build
- **Signer-cert accuracy:** initial version named the first cert in the bag
  (often the root CA). Now matches the SignerInfo `IssuerAndSerialNumber`
  against the certificate set, so it reports the actual end-entity signer
  (verified: `CN=John B Harris` issued by `GeoTrust CA for Adobe`, not
  `Adobe Root CA`).
- **Zero-padded `/Contents`:** PDF over-allocates the fixed-size signature
  buffer and right-pads with `0x00`; switched from strict `from_der` (which
  errored on trailing data) to a reader-based decode that reads one DER value
  and ignores the padding.
- **Honest integrity scope:** added the `integrity_check` field + a per-signature
  `note` so callers don't mistake the structural `/ByteRange` check for a full
  cryptographic verification.

## Out-of-model features (intentionally NOT built)
- **Full cryptographic digest verification** (recompute SHA over the byte ranges,
  verify the signed hash + RSA/ECDSA signature against the signer's public key).
  Feasible in pure Rust in principle, but it requires reconstructing the exact
  signed-attributes DER and per-algorithm signature checks; it is a substantially
  larger surface and is deliberately scoped out — the tool is an **inspector**,
  not a validator, and says so.
- **Certificate-chain trust building + root-store validation** — needs a bundled
  trust store and path-building; out of scope for a stateless tool.
- **Revocation (OCSP/CRL) and timestamp-token (RFC 3161) validation** — require
  network calls to CA infrastructure beyond the single document fetch.

These are listed, not built, per the improve-tool rule (no competitor
copy/branding reused).

## Verification (this build)
- `cargo test --workspace`: 15 tests pass (10 core + 5 block, incl. the
  drift-guard schema test).
- `wafer build`: chat block instantiates in wasm32-wasip1 (lopdf + cms +
  x509-cert + der all instantiate) — 1.4 MiB block.wasm.
- CLI (`gizza tool pdf-signature-inspector url=…`):
  - signed PDF → full signer/issuer/time/sub-filter/byte-range output;
  - unsigned PDF → `{"signed":false,...}`;
  - non-PDF URL → clean content-type error.
- No page surface (binary file input → JSON), as documented; no Playwright spec.

Sources: pdfsigncheck.com, esign.ai/tool/verifysignature,
devtoolcafe.com/tools/pdf-signature, ireadpdf.com/validate-signature,
helpx.adobe.com/acrobat (validate-digital-sign).

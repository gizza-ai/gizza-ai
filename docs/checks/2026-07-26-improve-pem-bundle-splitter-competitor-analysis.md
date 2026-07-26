# pem-bundle-splitter — competitor analysis (2026-07-26)

Tool function: split a multi-block PEM bundle (a `.pem`/`fullchain.pem`-style file
holding several `-----BEGIN …-----` blocks) into individual labeled blocks and report
the **type** and **order** of each. Built pure-Rust/wasm (browser-local, no upload).

## Competitors scanned (paraphrased — no copy/branding reused)

1. **JavaInUse — Online PEM Bundle Splitter** (javainuse.com/pemsplitter). The closest
   match. Paste PEM text or upload a file; a "Split Bundle" action decomposes the bundle
   into separate certificate files. Offers two **naming schemes**: numbered
   (`cert_1.pem`) or by **Common Name**. Focused on splitting certificate chains; the
   page doesn't surface per-block type classification, byte counts, or fingerprints.
2. **Orbit2x — PEM File Decoder** (orbit2x.com/pem-decoder). A *decoder* (not a splitter):
   accepts `.pem/.crt/.cer/.key/.csr/.pub` via paste or upload. Recognizes four block
   classes — X.509 certificates, private keys (PKCS#1/PKCS#8/SEC1), public keys (PKIX),
   and CSRs (PKCS#10). Per block it deep-decodes subject/issuer DN, validity, serial,
   signature algorithm, key algorithm/bit length, extensions (SAN, key usage), and
   SHA-256/SHA-1/MD5 fingerprints; does chain-signature validation and flags
   weak/expired keys. JSON export.
3. **Monocalc / InternetSecure / CertificateDecoder.dev — Certificate Decoders**. Paste a
   full bundle; each certificate is decoded separately and **labelled by its position in
   the chain**. Report issuer/subject/expiry/SAN/key size/fingerprints. Multi-cert /
   full-chain input supported. Decoder class, cert-only.
4. **CLI idioms** (jvt.me, gists): `csplit`/`awk` on `-----BEGIN CERTIFICATE-----`,
   `split -p`, and the Python `pem` library (parse + split any block type). These are the
   canonical "how do I split a bundle" answers our tool replaces with one paste.
5. **JavaInUse pem2crt / Convert .pem to .crt & .key**. Separates a combined PEM into its
   cert vs key components for import into keystores/servers.

## Table-stakes → disposition

| Capability | Source | In-model? | Disposition |
|---|---|---|---|
| Split bundle into individual `-----BEGIN…-----` blocks | all | yes | **Built** (core split) |
| Handle **any** block type, not just certs (keys, CSRs, params, PGP, OpenSSH) | pem lib, orbit2x | yes | **Built** — reads the PEM label of every block |
| Label each block by **type** (X.509 cert, PKCS#8/PKCS#1/SEC1 key, PKCS#10 CSR, …) | orbit2x, monocalc | yes | **Built** — friendly type + PKCS/format naming from the label |
| Report **order / position** in the bundle ("block N of M", chain order) | monocalc, InternetSecure | yes | **Built** — 1-based index + "N of M" |
| Summary counts by category (certs / keys / CSRs / other) | decoders | yes | **Built** — summary line + JSON summary object |
| Per-block **byte length** (DER size) | — (adds value) | yes | **Built** — DER byte length + base64 body chars |
| **SHA-256 fingerprint** of each block's DER (pinning / dedupe / identify) | orbit2x, monocalc | yes | **Built** — optional `fingerprints` (default on) |
| Re-emit each block as clean, individually copy-pasteable PEM | splitters | yes | **Built** — normalized 64-col PEM per block; `output=pem` mode |
| JSON export for scripting | orbit2x | yes | **Built** — `output=json` (structured array) |
| Numbered file-name suggestions (`cert_1.pem`) | javainuse | yes | **Built** — suggested filename per block (numbered scheme) |
| Naming **by Common Name** | javainuse | no (needs X.509 ASN.1 parse of subject DN) | **Out-of-model here** — see below |
| Deep X.509 decode: subject/issuer DN, validity, serial, SAN, key usage, sig alg | orbit2x, monocalc, decoders | partial / different tool | **Out-of-scope** — this slug is the *splitter*; full field decode is the decoder-class tool. Listed, not built. |
| Chain-signature validation / weak-key & expiry flags | orbit2x | needs full X.509 + trust logic | **Out-of-model** — listed, not built |
| SHA-1 / MD5 fingerprints | decoders | yes but deprecated | **Considered, rejected** — SHA-256 only; SHA-1/MD5 are deprecated for pinning and add noise |
| File upload | splitters/decoders | yes (page paste covers it) | Page accepts pasted text (multiline); the browser-local model needs no upload |

## Out-of-model / out-of-scope (considered, not built)

- **Common-Name-based naming & deep certificate field decode** (subject/issuer/validity/
  serial/SAN/key-usage/signature-algorithm). These require parsing the inner X.509 ASN.1
  (DER) of each certificate, which is the job of a dedicated *certificate decoder* tool,
  not a bundle splitter. This tool deliberately stays a **generic, label-driven splitter**
  that works for every block type (keys, CSRs, params, PGP, OpenSSH), not only certs —
  the same design choice `pem-der-convert` makes. Numbered filename suggestions are
  provided; CN-based names are noted as a decoder-tool feature.
- **Chain validation / trust-path / weak-key & expiry warnings** — need full X.509 parse
  plus a trust store and clock; out of a pure splitter's scope.

## Design decisions

- Params: `pem` (multiline bundle, required), `output` enum `report|json|pem` (default
  `report`), `fingerprints` boolean (default `true`, SHA-256 of each block's DER).
- Type map covers X.509 CERTIFICATE/TRUSTED CERTIFICATE/attribute cert, X509 CRL, PKCS#10
  CSR, PKCS#8 (ENCRYPTED) PRIVATE KEY, PKCS#1 RSA PRIVATE/PUBLIC KEY, SEC1 EC PRIVATE KEY,
  DSA key, PKIX PUBLIC KEY, EC/DH/DSA PARAMETERS, PKCS#7/CMS, PGP public/private blocks,
  OpenSSH private key, SSH2 public key; unknown labels fall through to a generic label.
- Tolerates human-readable text **between** blocks (the common `openssl x509 -text`
  bundle shape) — `pem::parse_many` skips interstitial text.
- No copy, branding, or trademarks reused from any competitor; all copy is original.

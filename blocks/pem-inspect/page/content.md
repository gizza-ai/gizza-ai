## About this tool

**PEM Inspector** decodes PEM-encoded X.509 certificates, PKCS#10 certificate
requests (CSRs), and public/private keys and shows you exactly what is inside
them — subject, issuer, validity window, Subject Alternative Names, key
algorithm and size, key usage, and SHA-256/SHA-1 fingerprints. Everything runs
in a locally compiled WebAssembly (WASM) binary, so **your certificates and keys
never leave your device**. You can paste a whole certificate chain at once —
each `-----BEGIN …-----` block is decoded independently.

A PEM file is just base64-wrapped DER with a `-----BEGIN <label>-----` header,
and the label tells you what the block is: `CERTIFICATE`, `CERTIFICATE REQUEST`,
`PUBLIC KEY`, `RSA PRIVATE KEY`, and so on. This tool reads that structure so you
don't have to reach for `openssl x509 -text` on the command line.

### Worked example

Paste a certificate block such as:

```
-----BEGIN CERTIFICATE-----
MIID… (base64 body) …QmA=
-----END CERTIFICATE-----
```

and the tool returns one JSON object describing it, for example:

```json
[
  {
    "type": "certificate",
    "version": "v3",
    "serial": "0a1b2c…",
    "subject": "CN=pem-inspect.example",
    "issuer": "CN=pem-inspect.example",
    "self_signed": true,
    "not_before": "Jul 29 13:23:39 2026 +00:00",
    "not_after": "Jul 24 13:23:39 2046 +00:00",
    "status": "valid",
    "days_until_expiry": 7144,
    "is_ca": false,
    "subject_alt_names": ["DNS:pem-inspect.example"],
    "public_key": { "algorithm": "RSA", "key_size_bits": 2048 },
    "signature_algorithm": "SHA-256 with RSA",
    "fingerprint_sha256": "AB:CD:…",
    "fingerprint_sha1": "12:34:…"
  }
]
```

The `status` and `days_until_expiry` fields are computed against your browser's
current time, so you can tell at a glance whether a certificate is valid, not yet
valid, or expired.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>Is it safe to paste a private key here?</summary>

Yes. All parsing happens locally inside WebAssembly in your browser — nothing is
uploaded to a server. For any private key the tool reports **only** the key type,
algorithm, and size (for example, "PKCS#8 PrivateKeyInfo, RSA, 2048 bits"). It
never reads, prints, or transmits the secret scalar (the private exponent or EC
private value). That said, treat production private keys with care and prefer a
throwaway or test key when you can.

</details>

<details>
<summary>What PEM block types are supported?</summary>

X.509 certificates (`CERTIFICATE`, `TRUSTED CERTIFICATE`, `X509 CERTIFICATE`),
PKCS#10 requests (`CERTIFICATE REQUEST`, `NEW CERTIFICATE REQUEST`), SPKI public
keys (`PUBLIC KEY`), PKCS#1 RSA public keys (`RSA PUBLIC KEY`), and private keys
in PKCS#8 (`PRIVATE KEY`), PKCS#1 (`RSA PRIVATE KEY`), and SEC1 (`EC PRIVATE
KEY`) form. An `ENCRYPTED PRIVATE KEY` block is recognised but reported as
encrypted — decrypt it first (for example with `openssl pkcs8`) before
inspecting.

</details>

<details>
<summary>Can I decode a whole certificate chain at once?</summary>

Yes. Paste every `-----BEGIN …-----` / `-----END …-----` block one after another
and the tool decodes each block independently, returning one JSON object per
block in order. This is handy for inspecting a leaf certificate together with its
intermediate and root.

</details>

<details>
<summary>How do I get the PEM text out of a `.pfx`/`.p12` or `.der` file?</summary>

This tool takes PEM (base64) text, not binary files. Convert a DER file with
`openssl x509 -inform der -in cert.der`, or extract certificates from a PKCS#12
bundle with `openssl pkcs12 -in bundle.p12 -nokeys -clcerts`, then paste the
resulting `-----BEGIN CERTIFICATE-----` block here.

</details>

## Limitations

- **Decoding only, not verification.** The tool describes what a block contains;
  it does **not** build or validate a chain of trust, check signatures against an
  issuer, or query revocation (CRL/OCSP). Those need a trust store and network
  access, which this browser-local tool deliberately avoids.
- **PEM text input.** Binary DER, `.pfx`/`.p12`, and JKS files are not read
  directly — convert them to PEM first (see the FAQ).
- **Encrypted private keys** are recognised but not decrypted; decrypt them
  outside the tool first.
- **Expiry is relative to your device clock.** `status` and `days_until_expiry`
  use the browser's current time, so an incorrect system clock will skew them.

## About this tool

PEM Bundle Splitter separates a concatenated `.pem`, `.crt`, `.csr`, or
`fullchain.pem` file into the individual `-----BEGIN ...-----` blocks it contains.
It keeps the original order, names each block by its PEM label, maps common labels
such as `CERTIFICATE`, `PRIVATE KEY`, and `CERTIFICATE REQUEST` to friendly types,
and reports DER byte sizes plus optional SHA-256 fingerprints.

Use it before installing a TLS chain, separating a combined certificate/key export,
or reviewing a mixed bundle without uploading sensitive material anywhere.

### Worked example

Input:

```pem
-----BEGIN CERTIFICATE-----
AQID
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE REQUEST-----
BAUG
-----END CERTIFICATE REQUEST-----
```

With `output = report` and fingerprints off, the report starts with:

```text
PEM bundle: 2 blocks
  certificate: 1 · certificate signing request: 1

Block 1 of 2
  type:     X.509 certificate
  label:    CERTIFICATE
```

Switch `output` to `pem` when you want normalized copy-pasteable blocks with
comment headers and suggested filenames, or `json` when another script should
consume the split result.

## Limits & edge cases

- This is a label-driven splitter. It does not deep-decode X.509 subjects,
  validity windows, SANs, or key algorithms.
- Unknown PEM labels are still preserved and reported as `other` instead of
  being rejected.
- Text before, after, or between PEM blocks is ignored if the PEM blocks
  themselves are complete.
- Fingerprints are SHA-256 over the decoded DER bytes, not over the armored text.
- The output may contain private keys if your input contains private keys. Treat
  copied output with the same care as the original bundle.

## FAQ

<details>
<summary>Can this split a full certificate chain?</summary>

Yes. Paste a `fullchain.pem` or any concatenated certificate chain and the tool
returns each `CERTIFICATE` block in the same order it appeared in the bundle.

</details>

<details>
<summary>Does it understand private keys and CSRs?</summary>

Yes. The type map covers PKCS#8 private keys, encrypted private keys, RSA and EC
private keys, public keys, certificate signing requests, parameters, PGP blocks,
OpenSSH private keys, and unknown labels.

</details>

<details>
<summary>Why is there no Common Name based filename?</summary>

Common Name naming requires deep X.509 ASN.1 parsing of certificate subjects.
This tool intentionally stays a generic splitter for certificates, keys, CSRs,
and other PEM labels. It provides stable numbered filenames instead.

</details>

<details>
<summary>Are fingerprints calculated from the certificate text?</summary>

No. The SHA-256 fingerprint is calculated from the decoded DER bytes inside each
PEM block. Whitespace or line wrapping changes in the PEM armor do not change the
fingerprint.

</details>

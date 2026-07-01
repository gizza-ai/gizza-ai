## About this tool

PEM and DER are two encodings of the **same** cryptographic objects — RSA/EC
private and public keys, X.509 certificates, certificate signing requests
(CSRs), and CRLs. **DER** is the raw binary ASN.1 form. **PEM** is just that DER,
base64-encoded and wrapped in `-----BEGIN <label>-----` / `-----END <label>-----`
armor. This tool converts between the two in either direction.

### How it works

- **PEM → DER:** paste a PEM block. The tool reads its label (e.g.
  `CERTIFICATE`, `PRIVATE KEY`, `EC PRIVATE KEY`, `CERTIFICATE REQUEST`),
  decodes the base64 body, and shows the resulting DER bytes as **hex** or
  **base64**, along with the detected label and byte length.
- **DER → PEM:** paste DER bytes as **hex** or **base64**, pick a PEM
  **label**, and the tool wraps them into a standard 64-column PEM block.

Because it is a **generic re-encoder**, it does not parse or validate the inner
ASN.1 — so it works for any object type, not just a fixed list of key formats.

### Privacy

Everything runs locally in your browser via WebAssembly. Your keys and
certificates are **never uploaded** anywhere.

### Tips

- DER input accepts `0x` prefixes and `:` / `-` / whitespace separators in hex.
- For `DER → PEM`, you can paste a full `-----BEGIN ...-----` line as the label
  and it will be extracted automatically; a blank label defaults to
  `CERTIFICATE`.

## FAQ

<details>
<summary>Can I convert a whole certificate chain in one go?</summary>

Yes. In PEM → DER mode the tool parses **every** `-----BEGIN ...-----` block
in the input, so a full chain (leaf + intermediates + root) comes back as one
DER result per block, each with its detected label and byte length.

</details>

<details>
<summary>How does the "auto" direction decide which way to convert?</summary>

It checks for a `-----BEGIN` header: if one is present the input is treated as
PEM and converted to DER; otherwise the input is decoded as DER bytes (hex or
base64, per the DER format setting) and wrapped into PEM. If your input is
ambiguous, pick `pem-to-der` or `der-to-pem` explicitly.

</details>

<details>
<summary>Does converting validate that my key or certificate is well-formed?</summary>

No. This is deliberately a generic re-encoder — it decodes the base64/hex and
re-wraps it without parsing the inner ASN.1. That is what lets it handle any
object type (keys, certs, CSRs, CRLs), but it also means a corrupted DER blob
will convert "successfully". Use `openssl asn1parse` if you need structural
validation.

</details>

<details>
<summary>What label goes into the -----BEGIN line for DER → PEM?</summary>

Whatever you type in the label field, uppercased — common ones are
`CERTIFICATE`, `PRIVATE KEY`, `EC PRIVATE KEY`, and `CERTIFICATE REQUEST`.
Pasting a full `-----BEGIN X-----` line works too (the armor is stripped), and
leaving it blank falls back to `CERTIFICATE`. Output is wrapped at the
standard 64 columns.

</details>

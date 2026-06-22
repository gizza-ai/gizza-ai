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

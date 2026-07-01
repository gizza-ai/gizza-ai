## About this tool

**ASN.1 / DER parser** takes an ASN.1 byte stream — encoded with the **Distinguished
Encoding Rules (DER)** and written as **hex** — and walks it into a readable tree of
**tag / length / value** elements. It's the format underneath X.509 certificates,
PKCS keys, CSRs, OCSP responses and much of the PKI world.

Paste hex with or without spaces, colons, newlines or a leading `0x` — separators are
ignored.

### What it decodes

- **Structural tags** — `SEQUENCE`, `SET`, and constructed context-specific tags
  (shown as `[0]`, `[1]`, …), recursed into as child nodes.
- **Primitives** — `INTEGER` (as decimal and hex), `BOOLEAN` (`TRUE`/`FALSE`),
  `NULL`, `ENUMERATED`, `BIT STRING`, `OCTET STRING`, and the string types
  (`UTF8String`, `PrintableString`, `IA5String`, `BMPString`, …).
- **Times** — `UTCTime` and `GeneralizedTime`.
- **OBJECT IDENTIFIERs** — shown in dotted form with a friendly name for common
  PKI/X.509 OIDs, e.g. `1.2.840.113549.1.1.11` → *sha256WithRSAEncryption*,
  `2.5.4.3` → *commonName (CN)*, `1.2.840.10045.3.1.7` → *prime256v1 (P-256)*.
- **Encoding details** — short- and long-form lengths, high-tag-number tags, and
  the class of each element (universal / application / context-specific / private).

### Encapsulated structures

Certificates nest DER inside a `BIT STRING` (the subject public key) or an
`OCTET STRING` (an extension value). When those contents parse cleanly as a nested
structure, the parser recurses into them and marks the node `[encapsulated]`.

### Output formats

- **tree** (default) — an indented text tree, one line per element with its tag,
  raw tag byte, length, offset and decoded value.
- **json** — the same parse tree as structured JSON, handy for scripting.

### Tips

- To decode a **PEM** certificate or key first, strip the `-----BEGIN…-----`
  armor, base64-decode the body to DER, and paste the DER as hex.
- Offsets (`@N`) are byte positions within the stream, so you can cross-reference
  with a hex editor.

### Privacy

Everything runs **in your browser** via WebAssembly — your data is **never
uploaded** to a server. Also available from the [gizza CLI](/) and in chat.

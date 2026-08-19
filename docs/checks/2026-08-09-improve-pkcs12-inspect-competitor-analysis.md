# pkcs12-inspect — competitor analysis (2026-08-09)

Scan run BEFORE implementing. All notes are paraphrased observations of publicly documented
behaviour; no competitor copy, branding, or trademarks are reproduced or reused.

## Search

One search: PKCS#12 / .p12 / .pfx online inspector, bag structure, friendly names, password-free
structural read. Three reachable tools/references skimmed.

## Competitors skimmed

1. **In-browser PKCS#12 parser (qcecuring.com/tools/pkcs12-parser)** — single file-upload control
   plus a **required password** field; parses locally with a JS crypto library and prints the
   certificates and key details found inside. Privacy claim: file and password stay on the device.
   No presets/examples, no documented size limits, one "parse" action button.
2. **Digital certificate inspector (flamecore.cloud/digital-certificate-inspector)** — accepts
   `.p12`/`.pfx` and bare `.cer`; advertises issuer, subject, validity window, fingerprint,
   public key, serial, extensions, and (for PKCS#12) the container's bag structure. Page is
   mostly marketing shell; field list taken from its own feature description.
3. **`openssl pkcs12 -info` (reference implementation of the CLI equivalent, per freekb.net
   article)** — prints MAC iteration count and MAC-verified status, the PKCS#7 layer's PBE
   algorithm with its iteration count (e.g. a SHA-1/40-bit-RC2 PBE at 2048 iterations), per-bag
   attributes (local key ID, friendly name), and each certificate's subject/issuer DN followed by
   the PEM body. Flags of note: `-info`, `-nokeys`, `-nocerts`, `-noout`, `-clcerts`, `-cacerts`,
   `-passin`.

## Table stakes → decision

| Table stake (seen at ≥1 competitor) | Fit | Where it landed |
| --- | --- | --- |
| Container version + high-level summary | in-model | `version`, counts in both text and JSON output |
| MAC digest algorithm, iteration count, salt length, presence | in-model | `mac` section (from `MacData`/`DigestInfo`) |
| PBE/encryption algorithm per protected SafeContents + iterations + salt length | in-model | per-`safe_contents` `encryption` block, PBES1 *and* PBES2 (KDF, PRF, cipher) |
| Bag inventory by type (cert / key / shrouded key / CRL / secret / nested) | in-model | `bags[].type` with the PKCS#12 bag OID resolved to a name |
| Friendly name (`friendlyName`, BMPString) | in-model | `bags[].friendly_name` |
| Local key ID (`localKeyID`, hex) | in-model | `bags[].local_key_id` |
| Certificate subject / issuer DN | in-model | cert bags in unencrypted SafeContents |
| Certificate serial, validity window, SHA-256 fingerprint, key algorithm | in-model | same cert-bag detail block (x509-parser, already proven wasm-safe) |
| Key-pair pairing hint (which cert matches which key) | in-model | shared `localKeyID` is surfaced so pairing is readable |
| Machine-readable output | in-model | `format = json` (text report is the default) |
| Hex *or* base64 input, autodetected | in-model | `encoding = auto\|base64\|hex` |
| Runs locally / nothing uploaded | in-model | pure Rust→wasm, same as every gizza block |
| Binary file drag-and-drop upload | **out-of-model** | pure-tool pages take text fields only; base64/hex paste is the supported form (`base64 file.p12` one-liner is documented on the page) |
| Password entry → decrypt bags, print private key / cert PEM | **out-of-model (deliberate scope)** | this tool answers "what is in here" without the password; decryption needs the PKCS#12 KDF plus legacy RC2/RC4/3DES PBE ciphers and would emit private-key material. Not built; the page says so and points at `openssl pkcs12` for extraction |
| MAC *verification* (`MAC verified OK`) | **out-of-model** | verification requires the password (it keys the HMAC), so only MAC parameters are reported |
| `-nokeys` / `-clcerts` style output filters | out-of-model | the whole inventory is small; filtering is the caller's job (JSON output makes it trivial) |
| Certificate X.509 extensions dump | out-of-model | already covered by `blocks/pem-inspect`; this tool stays a container-structure view |

## Not a duplicate

`blocks/pem-inspect` decodes PEM text blocks (certs/CSRs/keys); `blocks/asn1-parser` prints a
generic DER tree from hex; `blocks/pem-der-convert` re-encodes single objects. None of them read a
PKCS#12 container's `AuthenticatedSafe` → `SafeContents` → `SafeBag` structure, bag attributes, or
PBE/MAC parameters. Confirmed against each block's `core/src/lib.rs` before building.

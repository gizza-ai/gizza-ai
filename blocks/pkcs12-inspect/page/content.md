## What this tool shows you

A PKCS#12 file (`.p12`, `.pfx`) is a container: an *AuthenticatedSafe* holding one or more
*SafeContents* entries, each holding *SafeBags* — a certificate bag per certificate, a key bag per
private key, plus attributes such as `friendlyName` (the alias Windows, Java and macOS show you)
and `localKeyID` (the value that pairs a key with its certificate).

The bag *contents* are usually encrypted, but the container's **structure** is not. This inspector
reads that structure with **no password at all**: how many certificates and keys are inside, what
each bag is called, which encryption and iteration counts protect it, and what the integrity-MAC
parameters are. Certificates that sit in an unencrypted bag are decoded in full.

Nothing is uploaded — the parsing runs as WebAssembly inside your browser — and no key material is
ever printed. An encrypted private key is reported by algorithm only.

## Worked example

Get the container as base64 and paste it into the field:

```
base64 -w0 keystore.p12
```

Pasting the sample keystore (a self-signed P-256 certificate and its key, exported with
unencrypted bags) gives:

```
PKCS#12 container: version 3, 967 bytes
Integrity MAC: SHA-256, 2048 iterations (MAC 32 bytes, salt 8 bytes) — not verified (needs the password)
Contents: 1 certificate bag(s), 1 unencrypted key bag(s), 0 encrypted key bag(s), 0 other bag(s), 0 encrypted SafeContents
Password required to extract: no

SafeContents 1: data (1.2.840.113549.1.7.1)
  Bag 1: certBag (1.2.840.113549.1.12.10.1.3)
    friendlyName: EC Sample
    localKeyID: 45F5FCB2E17ECF8E9E95E91CE1FFE1733A8E4A56
    subject: C=US, O=Gizza Test, CN=ec.example.test
    issuer: C=US, O=Gizza Test, CN=ec.example.test
    serial: 03:06:3d:e9:08:57:45:e4:c6:85:e7:1b:29:25:ab:e2:98:a9:6c:26
    validity: Aug  9 18:47:30 2026 +00:00 .. Aug  6 18:47:30 2036 +00:00
    self-signed: true, CA: true
    public key: EC 256 bit
    signature: ECDSA with SHA-256
    SHA-256: 07:D6:B0:AD:0A:17:10:FC:59:7F:29:BD:4C:06:99:D7:2D:D5:4C:FE:CC:5B:19:31:64:1E:4C:CD:15:ED:33:9B

SafeContents 2: data (1.2.840.113549.1.7.1)
  Bag 1: keyBag (1.2.840.113549.1.12.10.1.1)
    friendlyName: EC Sample
    localKeyID: 45F5FCB2E17ECF8E9E95E91CE1FFE1733A8E4A56
    note: unencrypted PKCS#8 private key — anyone with this file has the key; key material is never printed
```

A password-protected keystore (the usual case) reports the protection instead of the bag list:

```
SafeContents 1: encryptedData (1.2.840.113549.1.7.6)
  Encryption: PBES2, PBKDF2, AES-256-CBC, 2048 iterations, PRF hmacWithSHA256
  Salt: 16 bytes
  Encrypted payload: 1008 bytes
  Note: password-protected: bag contents are encrypted and are not listed
```

Switch **Output** to JSON for the same information as a structured object (`version`, `mac`,
`safe_contents[].bags[]`, and a `summary` with the bag counts) — handy for scripting an audit of
which keystores still use a weak legacy cipher.

## Typical uses

- Checking a `.pfx` before importing it into a server: does it actually contain the private key, or
  only certificates?
- Finding the alias/`friendlyName` a keystore uses, so an import or a Java `keytool` command can
  reference it.
- Confirming which certificate belongs to which key by matching `localKeyID` values.
- Auditing protection strength: legacy exports use SHA-1-based PBE with 40-bit RC2, while modern
  ones use PBES2 with AES-256-CBC and a higher iteration count.
- Counting the chain: one leaf plus intermediates means the container carries a full chain.

## Limits and edge cases

- **Structure only.** Bag contents are never decrypted, so certificates hidden inside an encrypted
  SafeContents cannot be listed. Only bag-level metadata that lives outside the ciphertext (bag
  type, `friendlyName`, `localKeyID`, encryption parameters) is available for those.
- **The MAC is not verified.** The integrity MAC is keyed by the password, so its algorithm,
  iteration count and salt length are reported but correctness is not checked. A wrong-password
  message from another tool is not something this one can reproduce.
- **Input is base64 or hex**, not a file picker — pages here take text fields. `base64 -w0
  file.p12` (or `xxd -p file.p12` for hex) produces what to paste. Up to 4 MiB decoded is accepted.
- **PKCS#12 version 3 only** — every real-world file is version 3; anything else is rejected as
  malformed rather than guessed at.
- Public-key–integrity (`signedData` authSafe) and `envelopedData` SafeContents are recognised and
  reported, but their bags cannot be listed without the recipient's private key.
- A malformed SafeContents entry is reported inline; the rest of the container is still listed.

<details>
<summary>Do I need the keystore password?</summary>

No. The container's outer structure — version, MAC parameters, SafeContents entries, their
encryption algorithms, and each bag's type, `friendlyName` and `localKeyID` — is stored in the
clear. That is what this tool reads. The password would only be needed to decrypt bag *contents*,
which this tool deliberately never does.

</details>

<details>
<summary>Why can't I see the certificate details for my file?</summary>

Because its certificate bags are inside a password-encrypted `SafeContents`, which is what most
export tools produce by default. You will still see how many bags there are, their friendly names
and the cipher protecting them. To see the certificates themselves, decrypt the file with the
password first (for example `openssl pkcs12 -in file.p12 -nokeys -out certs.pem`) and inspect the
resulting PEM.

</details>

<details>
<summary>Is my private key at risk when I paste a keystore here?</summary>

The parsing happens locally in your browser as WebAssembly — the base64 you paste is not sent
anywhere. On top of that, key bags are only ever reported by type and algorithm: no private-key
bytes are printed, whether the key is encrypted or not. As always with secrets, prefer a machine
you trust.

</details>

<details>
<summary>What does localKeyID mean?</summary>

It is a short identifier stored as an attribute on both a key bag and its matching certificate
bag, so software can tell which key goes with which certificate when a container holds several.
Two bags sharing the same `localKeyID` are a pair. It is usually the certificate's SHA-1 key
identifier, but its exact value is producer-specific and should be treated as opaque.

</details>

<details>
<summary>What do "PBES2" and "pbeWithSHAAnd40BitRC2-CBC" tell me?</summary>

They are the password-based encryption schemes protecting the bags. `PBES2` with `PBKDF2` and
`AES-256-CBC` is the modern choice, and the reported iteration count shows how much work a
password guess costs. The `pbeWithSHAAnd…` names are the legacy PKCS#12 schemes — 40-bit RC2 in
particular is weak, and a keystore still using it is worth re-exporting.

</details>

<details>
<summary>Can it read a Java JKS or BKS keystore?</summary>

No. Those are different container formats with their own headers; this tool reads PKCS#12 only,
which is what modern `keytool` writes by default (and what `.p12`/`.pfx` files are). A JKS file
will be rejected because it does not begin with a DER `SEQUENCE`.

</details>

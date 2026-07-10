## About this tool

**Ed25519 Sign & Verify** does both halves of an Ed25519 signature in one place.
Pick **Sign** to turn a message + your private key into a signature, or **Verify**
to check a signature against a message and the signer's public key. Everything
runs in your browser via WebAssembly — your keys and messages are never uploaded.

Ed25519 is EdDSA over Curve25519 (RFC 8032): fast, 32-byte keys, 64-byte
signatures, and used by SSH, OpenPGP, JWT (`EdDSA`), and TLS.

### Worked example

Sign the single byte `0x72` with the RFC 8032 test key. Choose **Sign**, set
**Message** to `72`, **Message encoding** to **Hex**, and paste the raw private
key:

```
key = 4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb
```

You get back the deterministic signature and the derived public key:

```
signature (hex): 92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da
                 085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00
public key (hex): 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c
```

Switch to **Verify**, keep the same message, paste that **public key** into the
key box and the signature into the signature box, and you get `valid: true`.

### Options

- **Operation** — **Sign** (message + private key → signature) or **Verify**
  (message + public key + signature → valid / not valid).
- **Message encoding** — how the message box is read into bytes: **UTF-8 text**
  (default), **Hex**, or **Base64** (for signing binary payloads).
- **Key** — for signing, an Ed25519 **private** key; for verifying, the
  **public** key. Accepts PEM (`-----BEGIN PRIVATE KEY-----` PKCS#8 /
  `-----BEGIN PUBLIC KEY-----` SPKI), or a raw 32-byte key as **hex** or
  **base64**. A private key may also be a 64-byte `seed || public` keypair.
- **Signature** — only for verify: the raw 64-byte signature as hex or base64.

### Deterministic signatures

Ed25519 is deterministic by design (RFC 8032): the nonce is derived from the key
and message, so signing the same message with the same key always produces the
same signature. There is no RNG and no repeated-nonce risk.

### Limits & edge cases

- Keys are Ed25519 only — this is not ECDSA (P-256/P-384) or RSA. A raw key must
  decode to exactly **32 bytes** (public / private seed) or **64 bytes** (private
  keypair); signatures must decode to **64 bytes**.
- Verifying a signature that simply doesn't match returns `valid: false` — that's
  a normal result, not an error. Errors are reserved for malformed keys or
  signatures.
- Whitespace inside hex/base64 is ignored, so space-separated hex pastes fine.

### FAQ

<details>
<summary>What key formats can I paste?</summary>

For signing: an Ed25519 **private** key as PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`), or the raw 32-byte seed as hex or base64 (a 64-byte `seed || public` keypair also works). For verifying: the **public** key as SPKI PEM (`-----BEGIN PUBLIC KEY-----`) or the raw 32-byte point as hex or base64. The tool auto-detects hex vs base64 by length.

</details>

<details>
<summary>Why is the signature the same every time I sign?</summary>

That's expected. Ed25519 signatures are deterministic (RFC 8032) — the per-signature nonce is derived from the private key and message rather than a random number, so the same key + message always yields the same 64-byte signature. It removes the catastrophic key-leak risk that a repeated random nonce causes in ECDSA.

</details>

<details>
<summary>Verify says "valid: false" — what went wrong?</summary>

The signature doesn't match the message under that public key. Common causes: the message text or **message encoding** differs from what was signed (a trailing newline counts), the signature belongs to a different key, or you pasted the wrong public key. Double-check that the message bytes and public key are exactly the signer's.

</details>

<details>
<summary>Is this the same as ECDSA, RSA, or SSH signing?</summary>

No. Ed25519 is EdDSA over Curve25519 — a different scheme from ECDSA (NIST P-256/P-384) and RSA. It's the algorithm behind modern `ssh-ed25519` keys and JWT's `EdDSA`, but the raw signature here is not an SSH or PGP wire signature — it's the bare 64-byte Ed25519 signature.

</details>

<details>
<summary>Do my keys or messages leave the browser?</summary>

No. Signing and verifying run locally in WebAssembly; nothing is sent to a server. Still, treat any pasted production private key with care and prefer test keys where you can.

</details>

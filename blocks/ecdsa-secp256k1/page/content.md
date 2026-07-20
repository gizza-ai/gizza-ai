## About this tool

**ECDSA secp256k1** does all three halves of an elliptic-curve signature in one
place, on the curve Bitcoin and Ethereum use. Pick **Generate** for a fresh
keypair, **Sign** to turn a message + your private key into a signature, or
**Verify** to check a signature against a message and the signer's public key.
Everything runs in your browser via WebAssembly — keys and messages are never
uploaded.

secp256k1 is the Koblitz curve behind Bitcoin, Ethereum, and most other
blockchains: 32-byte private keys, 33/65-byte public points, 64-byte compact
signatures. Signing here is **deterministic (RFC 6979)** — no random nonce, no
repeated-nonce key-leak risk — and **low-S normalized**, the canonical form
Bitcoin and Ethereum require.

### Worked example

Sign `Satoshi Nakamoto` (UTF-8, SHA-256) with the private key `1` — the
widely-published RFC 6979 test vector. Choose **Sign**, type the message, and
paste the key:

```
key = 0000000000000000000000000000000000000000000000000000000000000001
```

You get the deterministic signature, its components, and the derived public key:

```
signature (compact hex): 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
                         2442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5
r: 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
s: 2442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5
recovery id: 0 (v = 27)
public key (compressed hex): 0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
```

Switch to **Verify**, keep the same message, paste that **public key** into the
key box and the compact signature into the signature box → `valid: true`.

### Options

- **Operation** — **Generate** (fresh keypair: hex + PEM, compressed +
  uncompressed public key), **Sign** (message + private key → signature), or
  **Verify** (message + public key + signature → valid / not valid).
- **Message encoding** — how the message box is read into bytes: **UTF-8 text**
  (default), **Hex** (`0x` ok), or **Base64**.
- **Hash** — the digest ECDSA signs: **SHA-256** (default, what Bitcoin-style
  message signing uses), **Keccak-256** (Ethereum's hash), **SHA-384**,
  **SHA-512**, or **None** when the message already *is* the 32-byte digest
  (e.g. a transaction hash — paste it as hex).
- **Key** — for signing, a secp256k1 **private** key: raw 32 bytes as hex
  (`0x` optional) or base64, or PEM (SEC1 `-----BEGIN EC PRIVATE KEY-----` /
  PKCS#8 `-----BEGIN PRIVATE KEY-----`). For verifying, the **public** key:
  SEC1 point — 33-byte compressed (`02…`/`03…`) or 65-byte uncompressed
  (`04…`) — as hex or base64, or SPKI `-----BEGIN PUBLIC KEY-----` PEM.
- **Signature** — only for verify: compact 64-byte `r||s` or ASN.1 DER, as hex
  or base64 (auto-detected).

### Signature output

Signing emits every common representation at once: compact `r||s` (hex and
base64), ASN.1 **DER** (what OpenSSL emits/consumes), the **r** and **s**
components, and the **recovery id** with the Ethereum-style `v = 27 + id` —
enough to feed wallets, `ecrecover`, or `openssl dgst -verify` without
conversion round-trips.

### Limits & edge cases

- secp256k1 only. For NIST P-256/P-384 signing use the ECDSA P-256/P-384
  signing tool; for Ed25519 use the Ed25519 sign & verify tool. Brainpool
  curves and ECDH key agreement are not supported.
- A raw private key must be exactly **32 bytes**, non-zero and below the curve
  order; a raw public key must be a valid **33/65-byte** SEC1 point.
- With **hash = None** the message must decode to exactly **32 bytes**.
- Verifying a signature that simply doesn't match returns `valid: false` —
  that's a normal result, not an error. High-S signatures are low-S normalized
  before checking (and flagged), so OpenSSL-made signatures verify fine.
- An empty message can't be signed — the tool asks for a message instead of
  silently signing the empty string.
- This signs raw digests; it does not build Bitcoin `signmessage` envelopes,
  Ethereum EIP-191 `personal_sign` prefixes, or EIP-155 transaction `v` values.

### FAQ

<details>
<summary>What key formats can I paste?</summary>

For signing: a secp256k1 **private** key as raw 32-byte hex (`0x` prefix ok, whitespace ignored), base64, or PEM — both SEC1 (`-----BEGIN EC PRIVATE KEY-----`, what `openssl ecparam -genkey` emits) and PKCS#8 (`-----BEGIN PRIVATE KEY-----`). For verifying: the **public** key as a SEC1 point — compressed 33-byte (`02…`/`03…`) or uncompressed 65-byte (`04…`) hex or base64 — or SPKI PEM. Hex vs base64 is auto-detected.

</details>

<details>
<summary>Why is the signature the same every time I sign?</summary>

That's RFC 6979 deterministic signing: the nonce is derived from your key and the message digest instead of a random number, so the same key + message always produces the same signature. It removes the catastrophic nonce-reuse failure that has leaked real ECDSA private keys, and it means no RNG is needed to sign. The signature is still verified by any standard ECDSA implementation.

</details>

<details>
<summary>Which hash should I pick for Bitcoin or Ethereum?</summary>

Ethereum hashes messages with **Keccak-256**, so pick that to match Ethereum tooling (note: wallets' `personal_sign` also prepends an EIP-191 prefix before hashing, which this tool does not add). Bitcoin-ecosystem tooling generally signs **SHA-256** digests (Bitcoin's `signmessage` adds its own envelope and double-hashing). To sign an exact 32-byte digest you already have — a transaction hash, a sighash — pick **None** and paste the digest as hex.

</details>

<details>
<summary>What are the recovery id and v for?</summary>

An ECDSA signature alone doesn't say *which* public key made it — up to four candidate keys match. The recovery id (0-3) picks the right one, which is how Ethereum's `ecrecover` gets the signer's address from just the signature. `v = 27 + recovery id` is the legacy Ethereum encoding of the same bit (EIP-155 transactions fold the chain id into `v`; this tool reports the plain form).

</details>

<details>
<summary>Verify says "valid: false" — what went wrong?</summary>

The signature doesn't match the message digest under that public key. Check that the **hash** matches what the signer used (a SHA-256 signature never verifies under Keccak-256), that the message bytes are identical (encoding and trailing newlines count), and that the public key really is the signer's. DER vs compact form does *not* matter — both are accepted — and high-S signatures are normalized automatically.

</details>

<details>
<summary>Is a key generated here safe to use for real funds?</summary>

The key is generated locally from your device's cryptographically secure RNG and never leaves the browser, and the math is a well-reviewed pure-Rust implementation. That said, for wallets holding real value the standard advice applies: prefer audited wallet software or hardware wallets, which add seed phrases, backups, and physical key isolation. This tool is ideal for development, testing, and learning.

</details>

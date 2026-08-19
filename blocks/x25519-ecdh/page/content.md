## About this tool

X25519 ECDH runs an elliptic-curve Diffie-Hellman key agreement on Curve25519, the exchange behind TLS 1.3, WireGuard, Signal, SSH and age. Paste your private key and the other party's public key and the tool returns the 32-byte shared secret both sides compute independently — plus the public key derived from the private key you pasted, so you can confirm you handed the peer the right half.

Keys are read in every form real tooling emits: 64 hex characters (a leading `0x` is fine), 32 raw bytes as standard or URL-safe base64, or an RFC 8410 PEM block — PKCS#8 `-----BEGIN PRIVATE KEY-----` for the private half, SubjectPublicKeyInfo `-----BEGIN PUBLIC KEY-----` for the public half. Turn on **Also show PKCS#8 / SPKI PEM** to get the same keys back in the PEM forms OpenSSL and most libraries import. Choose hex, base64 or URL-safe base64 for the output.

The raw X25519 output is a curve point, not a uniformly random key, so it should not be used directly as an AES or ChaCha20 key. The **Key derivation** control expands it with HKDF-SHA256 or HKDF-SHA512 (RFC 5869, with a salt, a context label and a chosen output length) or hashes it with a single SHA-256. HKDF-SHA256 is the default; `none` reports the raw RFC 7748 value and says in the output itself that it is not a finished key.

Both key fields are optional. Leave the private key empty to generate a fresh pair from the browser's CSPRNG. Leave the peer public key empty as well and a demo peer pair is generated too, so you can see one complete exchange — both private keys, both public keys and the secret they agree on — without opening a second tab.

Everything runs in WebAssembly on your own machine. No key, secret or derived value is uploaded, logged or stored.

## Worked example

The canonical RFC 7748 §6.1 test vector. Enter Alice's private key and Bob's public key:

```text
private_key:     77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a
peer_public_key: de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f
kdf:             none
```

The result reproduces the values printed in the RFC:

```text
Your public key    8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a
Peer public key    de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f

Shared secret      4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742
```

Now swap the sides — Bob's private key `5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb` with Alice's public key `8520f0…` — and the shared secret is identical. That symmetry is the whole point of the exchange.

To turn that secret into a key you can actually encrypt with, set `kdf` to `hkdf-sha256`, give the context label both sides agree on, and pick a length:

```text
kdf:        hkdf-sha256
kdf_salt:   handshake salt
kdf_info:   app v1 chat key
kdf_length: 32
```

Both parties who run these same settings over the same agreement get byte-identical derived keys.

## Security notes and limits

- **The raw shared secret is not a key.** Run it through HKDF (or at minimum SHA-256) before using it with a cipher. The tool warns in its own output whenever `kdf` is `none`.
- **Low-order peer keys are rejected.** A peer public key of small order forces the shared secret to all zeros — a value anyone can predict. That case returns an error naming the cause instead of a result.
- **Do not put a private key in a URL.** The page accepts `?param=` deep links for convenience; a link containing a private key ends up in history, referrers and server logs.
- **X25519 only.** Ed25519 keys sign, they do not do key agreement; a pasted Ed25519 key is refused by OID. Other curves (NIST P-256/384/521, secp256k1, brainpool) are a different tool's scope.
- **No message encryption here.** Feed the derived key into a symmetric cipher tool to encrypt something with it.
- **Ephemeral is safer.** A key pair generated here exists only for the run; nothing is stored, so copy anything you want to keep before leaving the page.

## FAQ

<details>
<summary>Why does my derived key differ from my peer's when the shared secret matches?</summary>

HKDF has three inputs, and only one of them comes from the curve. The salt, the info/context label and the output length must all be identical on both sides. A different `kdf_info` string — even a trailing space — produces a completely different key from the same shared secret. That is the feature: it lets one agreement produce separate keys for separate purposes.

</details>

<details>
<summary>What should I put in the HKDF info field?</summary>

A short, stable label describing what the key is for, such as `app v1 chat key` or `file encryption v2`. Some protocols also bind the exchange to its participants by including both public keys in this field; that works here too, as long as both sides build the string exactly the same way and in the same order.

</details>

<details>
<summary>Can I paste an OpenSSL-generated PEM key?</summary>

Yes. RFC 8410 X25519 keys are accepted directly: PKCS#8 for the private key, SubjectPublicKeyInfo for the public key. Enabling the PEM option also prints your keys back in those forms, so you can round-trip into OpenSSL or a language library without converting encodings by hand.

</details>

<details>
<summary>Why was my peer's public key rejected as a low-order point?</summary>

Curve25519 contains a handful of points whose order is very small. Multiplying any private key by one of them yields an all-zero shared secret, so an attacker who sends such a "public key" learns the secret in advance. The exchange checks for this and refuses the result. Ask the peer for a genuine public key rather than working around the error.

</details>

<details>
<summary>How long should the derived key be?</summary>

Match the algorithm you will feed it. 32 bytes is right for AES-256 or ChaCha20-Poly1305; 16 bytes for AES-128. Deriving 44 bytes in one call is a common trick for getting a 32-byte key plus a 12-byte nonce together. The HKDF ceiling is 8160 bytes (255 × the hash length).

</details>

<details>
<summary>Is X25519 the same as Ed25519?</summary>

No. They use related curves but do different jobs: X25519 performs key agreement, Ed25519 produces signatures. They are not interchangeable, and reusing one key pair for both is discouraged. Pasting an Ed25519 key here returns an error that names the OID so you can tell which key you grabbed.

</details>

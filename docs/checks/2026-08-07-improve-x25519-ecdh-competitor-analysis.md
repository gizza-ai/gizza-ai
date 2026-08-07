# x25519-ecdh — competitor analysis (2026-08-07)

Scan run BEFORE implementing, per `/create-next-tool` step 4. All notes are **paraphrased**
observations of behaviour and parameters; no competitor copy, branding, or trademarks are
reproduced. Sources are named only to identify what was examined.

Search: "X25519 ECDH shared secret online tool generate key pair derive".

## Competitors examined

### 1. 8gwifi.org — EC key exchange & encryption (a real hosted tool)

- Interactive ECDH playground built around an Alice/Bob pedagogy: it generates a key pair for each
  side, shows both public and private halves, then computes the shared secret from one side's
  private key and the other side's public key.
- Curve is a dropdown of 20+ named curves (NIST P-256/384/521, secp256k1, brainpool, sect/K/B
  binary curves, sm2p256v1, FRP256v1); P-256 is the landing default. Curve25519/X25519 is **not**
  in that list — the tool is Weierstrass-curve oriented.
- Adds an encrypt/decrypt step on top of the agreement: a message field plus an operation toggle,
  where encryption uses one side's private key with the other's public key and decryption swaps the
  roles.
- Keys and results are shown in text areas with download links; results can also be shared through
  a URL that embeds the message and keys (the page itself flags that as a risk).

### 2. `cryptography` (pyca) X25519 documentation

- Minimal, opinionated API surface: generate a private key, take the peer's public key, call
  `exchange(peer_public_key)` → 32 raw bytes.
- Serialization is the interoperability contract: private keys as **raw 32 bytes** or
  **PKCS#8** (PEM/DER); public keys as **raw 32 bytes** or **SubjectPublicKeyInfo** (PEM/DER).
  Convenience raw accessors exist for both halves.
- The worked example ends by pushing the raw agreement output through **HKDF-SHA256** with a salt
  and an info/context label; the docs explicitly warn that the raw shared secret should not be used
  directly as a key for most applications.
- Advises a fresh ephemeral private key per handshake.

### 3. Monocypher X25519 manual

- Confirms the fixed 32-byte sizes for secret key, public key, and shared secret, and that the
  scalar is clamped and the public key's top bit ignored.
- Documents the **all-zero output check**: protocols needing contributory behaviour must compare the
  result against an all-zero buffer and abort, otherwise an untrusted peer can force a shared secret
  that is a known constant (low-order point attack).
- Recommends hashing the raw secret together with **both public keys** (`shared_secret || your_pk ||
  their_pk`) rather than using it directly, and warns against reusing one secret key for both key
  agreement and signatures.

### 4. RFC 7748 (spec, used for test vectors)

- §6.1 gives the canonical Alice/Bob Diffie-Hellman vector; §5.2 gives single-iteration scalar ×
  u-coordinate vectors. These are the exactness anchors for our unit tests, CLI check, and
  Playwright assertions.

## Table stakes → in-model / out-of-model

| # | Table stake observed | Verdict | Where it lands |
|---|---|---|---|
| 1 | Derive a shared secret from *my private key* + *peer public key* | **in-model** | the tool's core operation |
| 2 | Generate a key pair when the user has none | **in-model** | `private_key` left empty → fresh CSPRNG key pair |
| 3 | Alice/Bob "see the whole exchange" demo view | **in-model** | `peer_public_key` left empty → a demo peer pair is generated and both halves are shown |
| 4 | Raw 32-byte keys in hex | **in-model** | `encoding = hex` (default) |
| 5 | Raw 32-byte keys in base64 (and URL-safe base64, which WireGuard/JWK-style flows paste) | **in-model** | `encoding = base64 \| base64url` |
| 6 | PKCS#8 private / SPKI public PEM in and out (the pyca interop contract) | **in-model** | accepted on input for both fields; emitted with `include_pem = true` |
| 7 | HKDF over the raw secret with salt + info, chosen output length | **in-model** | `kdf = hkdf-sha256 \| hkdf-sha512`, `kdf_salt`, `kdf_info`, `kdf_length` |
| 8 | Plain hash of the secret as the cheap alternative | **in-model** | `kdf = sha256` |
| 9 | Loud "don't use the raw secret as a key" guidance | **in-model** | printed with every `kdf = none` result, plus page copy and an FAQ |
| 10 | Low-order / all-zero shared-secret rejection | **in-model** | contributory check → actionable error naming the cause |
| 11 | Show that both sides compute the same value | **in-model** | worked example on the page uses the RFC 7748 §6.1 pair in both directions |
| 12 | Multi-curve support (P-256/384/521, secp256k1, brainpool, binary curves) | **out-of-model for this tool** | different tool scope; this tool is X25519-only by name. Other curves are already served by the ECDSA/secp256k1 blocks |
| 13 | Encrypt/decrypt a message with the agreed key | **out-of-model here** | belongs to the existing symmetric-encryption tools (feed the derived key into `aes-cipher` / `nacl-secretbox-encrypt`); folding a cipher in would double the schema |
| 14 | Share a URL that embeds the private key | **considered, rejected** | our pages do support `?param=` deep links, but we will not advertise putting a private key in a URL; the page copy warns against it instead |
| 15 | Hash `secret ‖ your_pk ‖ their_pk` transcript binding (Monocypher's advice) | **in-model, delivered as guidance** | `kdf_info` is the documented place to bind context; the FAQ shows pasting both public keys into it. Auto-binding would silently break interop with peers that hash differently |
| 16 | Downloadable key files | **already platform** | every text page ships Copy + Download from the shared generator |

## UX patterns worth taking

- Label the two sides explicitly (yours vs the peer's) instead of "key 1 / key 2" — the Alice/Bob
  framing is what makes ECDH legible to first-timers.
- Show the public key you derive from the private key the user pasted, so they can check they gave
  the peer the right half.
- Preset chips beat prose for a spec vector: an example chip that loads the RFC 7748 pair lets a
  user verify the tool against the standard in one click.
- State the "raw secret is not a key" caveat *in the output*, not only in the docs — hosted tools
  that only mention it in a paragraph get it ignored.

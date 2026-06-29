## About this tool

**SM2 public key from private key** derives the public key for a private key on
the **SM2** elliptic curve — the Chinese national standard **GM/T 0003** scheme
mandated by OSCCA, using curve **sm2p256v1**. The public point is computed as
`Q = d·G` (one scalar multiplication of the curve's base point), so the result is
fully **deterministic** — the same private key always yields the same public key.

- **Input:** a raw 32-byte private **scalar in hex** (64 hex chars, an optional
  `0x` prefix is fine) or a **PKCS#8 PEM** (`-----BEGIN PRIVATE KEY-----`). Leave
  the format on **auto** to detect which, or force **hex** / **pem**.
- **Output:** choose **all** for a labelled summary of every encoding, or pick a
  single one — **uncompressed** SEC1 hex (`04 || x || y`, 130 chars),
  **compressed** SEC1 hex (`02|03 || x`, 66 chars), or **SPKI PEM**
  (`-----BEGIN PUBLIC KEY-----`). The affine **x** and **y** coordinates are also
  shown.

### Privacy

Everything runs **in your browser** via WebAssembly — your private key never
leaves the device. The tool only ever returns **public** key material; the
private key is used solely to compute the public point and is never echoed back.
Also available from the [gizza CLI](/) and in chat.

### Need a new key instead?

If you don't already have a private key, use the **sm2-keypair-generate** tool to
create a fresh SM2 key pair (private + public, PEM + hex) with a secure random
number generator.

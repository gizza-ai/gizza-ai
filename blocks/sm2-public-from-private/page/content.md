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

## FAQ

<details>
<summary>What exactly can I paste as the private key?</summary>

Either a raw 32-byte scalar as **exactly 64 hex characters** (a leading `0x`
is fine — anything else in length is rejected), or a **PKCS#8 PEM** block
starting with `-----BEGIN PRIVATE KEY-----`. With the input format on
**auto** the tool detects which one you pasted; set it to `hex` or `pem` to
force the interpretation.

</details>

<details>
<summary>Compressed or uncompressed — which output should I use?</summary>

They encode the same point: uncompressed SEC1 is `04 ‖ x ‖ y` (130 hex
chars), compressed is `02`/`03` `‖ x` (66 hex chars, the prefix records y's
parity). Use whichever your target system expects — many GM/T toolchains
default to uncompressed — or pick **all** to get both plus the SPKI PEM and
the raw x/y coordinates.

</details>

<details>
<summary>Can I feed in a NIST P-256 private key?</summary>

A 32-byte P-256 scalar will be *accepted* (it's just a number in range), but
the point is computed on **sm2p256v1**, SM2's own curve — so the result is a
valid SM2 public key, **not** your P-256 public key. The two curves are
different; use an SM2 key with this tool.

</details>

<details>
<summary>Is my private key ever exposed?</summary>

No. Derivation runs entirely in your browser via WebAssembly, and the tool
only returns **public** key material — the private scalar is used for the
single `Q = d·G` multiplication and never echoed back or transmitted.

</details>

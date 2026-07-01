## About this tool

AES Key Wrap protects one cryptographic key with another. You supply a **key-encryption
key (KEK)** and the **key material** you want to wrap; the tool returns a wrapped blob that
is 8 bytes longer than the (padded) input and carries a built-in integrity check. Unwrapping
reverses the process and fails loudly if the KEK, algorithm, or data is wrong — so a corrupted
or tampered blob never silently decrypts to garbage.

Everything runs locally in your browser via WebAssembly. Your keys are never uploaded.

## KW vs KWP

- **KW (RFC 3394 / NIST SP 800-38F)** — the classic algorithm. The key material must be a
  non-empty multiple of 8 bytes and at least 16 bytes. Use it to wrap a 128/192/256-bit
  symmetric key.
- **KWP (RFC 5649, "key wrap with padding")** — wraps key material of *any* length from 1
  byte up, padding internally. Use it when your key isn't an 8-byte multiple.

## Key sizes

The length of the KEK selects the AES variant automatically:

- **16 bytes → AES-128**
- **24 bytes → AES-192**
- **32 bytes → AES-256**

Provide the KEK, the key material, and the wrapped output as **hex** or **base64** (your choice).

## When to use it

Key wrapping is how key-management systems store a data key under a master key (KEK): the data
key is wrapped and stored next to the data, and only an operator holding the KEK can unwrap it.
It is also the AES-KW / AES-KWP construction used inside JOSE/JWE (`A128KW`, `A256KWP`, …) and
PKCS#11.

If you want to encrypt arbitrary **plaintext** rather than another key, use the
[AES cipher](/tools/aes-cipher/) tool (CBC/CTR/GCM/ECB) or, for passphrase-based encryption with
a random salt and nonce, [text encrypt](/tools/text-encrypt/).

## Notes

- Wrapping is deterministic — wrapping the same key material under the same KEK always yields the
  same blob (there is no IV/nonce to supply).
- The integrity check is what makes unwrap safe: a wrong KEK or a flipped bit yields an error,
  not a wrong key.
- This is a low-level primitive. The KEK must itself be a strong, randomly generated key.

## FAQ

<details>
<summary>Why does KW reject my key material?</summary>

KW (RFC 3394) only accepts key material that is a non-empty multiple of 8 bytes and at
least 16 bytes long — it was designed to wrap AES-sized keys. If your input is, say, 10
or 33 bytes, switch the algorithm to **kwp** (RFC 5649), which pads internally and
accepts any length from 1 byte up.

</details>

<details>
<summary>Why do I get the exact same wrapped blob every time?</summary>

That's by design. AES Key Wrap is deterministic — there is no IV or nonce, so wrapping
the same key material under the same KEK always produces the same output. This is
normal for key wrapping and does not weaken it, because key material is (or should be)
high-entropy random bytes, unlike ordinary plaintext.

</details>

<details>
<summary>What does "integrity check failed" mean when unwrapping?</summary>

The 8-byte integrity check built into the wrapped blob didn't verify. That means the
KEK is wrong, the algorithm doesn't match the one used to wrap (kw vs kwp), the
encoding is set incorrectly (hex vs base64), or the blob was corrupted. The tool never
returns a "best guess" key — a failed check is always an error.

</details>

<details>
<summary>Do my keys leave my machine?</summary>

No. The wrap and unwrap operations run entirely in your browser via WebAssembly. The
KEK, the key material, and the wrapped output never touch a server.

</details>

## About this tool

**GOST Kuznyechik cipher** is a low-level encrypt/decrypt tool for the GOST R
34.12-2015 "Kuznyechik" block cipher (also specified in **RFC 7801**), the modern
Russian standard symmetric cipher. You supply the raw **key** and **IV/nonce**,
pick the **mode**, and get the result — handy for implementing or testing against
the standard, debugging interop, or learning how the cipher works.

- **Block / key:** Kuznyechik uses a 128-bit (16-byte) block and a fixed **256-bit
  (32-byte) key**.
- **Modes:** `CBC` (default), `CTR`, `CFB`, `OFB`, and `ECB` (insecure, reveals
  patterns — included for completeness only). CBC and ECB use **PKCS7** padding;
  CTR/CFB/OFB are stream modes (no padding).
- **Encoding:** key, IV and ciphertext are **hex** or **base64**; the plaintext is
  UTF-8 text. CBC/CTR/CFB/OFB need a 16-byte IV; ECB needs none.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

### Not sure which tool you want?

If you just want to **protect a message with a passphrase** (and have the salt,
key derivation and nonce handled safely for you), use the **text-encrypt** tool
instead — `gost-kuznyechik-cipher` is for when you already have a specific raw
key, IV and mode.

## FAQ

<details>
<summary>What key and IV sizes does Kuznyechik require here?</summary>

The key must be exactly 32 bytes (256 bits) and the IV exactly 16 bytes — both
supplied in the encoding you pick (`base64` by default, or `hex`). CBC, CTR,
CFB and OFB all need the IV; ECB takes none. Anything else is rejected with an
error rather than silently truncated.

</details>

<details>
<summary>Can I type a password instead of a raw key?</summary>

No — this is a low-level cipher tool, so the key field expects the raw 256-bit
key itself, base64- or hex-encoded, not a passphrase. If you want
passphrase-based encryption with salt, key derivation and nonce handled for
you, use the text-encrypt tool instead.

</details>

<details>
<summary>Which modes pad the plaintext, and which don't?</summary>

CBC and ECB are block modes and apply PKCS7 padding, so the ciphertext is
rounded up to a multiple of the 16-byte block. CTR, CFB and OFB are stream
modes — no padding, and the ciphertext is exactly as long as the UTF-8
plaintext.

</details>

<details>
<summary>Why is ECB mode listed if it's insecure?</summary>

For completeness and interop testing against the GOST R 34.12-2015 / RFC 7801
spec. ECB encrypts each 16-byte block independently, so identical blocks
produce identical ciphertext and patterns leak. For anything real, prefer CBC
or CTR with a fresh random IV.

</details>

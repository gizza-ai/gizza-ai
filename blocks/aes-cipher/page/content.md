## About this tool

**AES cipher** is a low-level AES encrypt/decrypt tool for developers: you supply
the raw **key** and **IV/nonce**, pick the **mode**, and get the result — handy for
implementing or testing against a spec, debugging interop, or learning how the
modes differ.

- **Modes:** `GCM` (authenticated — detects tampering, recommended), `CBC`, `CTR`,
  and `ECB` (insecure, reveals patterns — included for completeness only).
- **Key sizes:** AES-128/192/256, selected automatically by the key length
  (16/24/32 bytes).
- **Encoding:** key, IV and ciphertext are **hex** or **base64**; the plaintext is
  UTF-8 text. For GCM the 16-byte authentication tag is appended to the ciphertext.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

### Not sure which tool you want?

If you just want to **protect a message with a passphrase** (and have the salt,
key derivation and nonce handled safely for you), use the **text-encrypt** tool
instead — `aes-cipher` is for when you already have a specific raw key, IV and
mode.

## FAQ

<details>
<summary>Why do I get "AES key must be 16/24/32 bytes"?</summary>

The key is decoded from hex or base64 first (per the **format** setting), and the *decoded* byte length selects the strength: 16 bytes = AES-128, 24 = AES-192, 32 = AES-256. A 32-character hex string is only 16 bytes — for AES-256 in hex you need 64 hex characters. A plain password is not a valid key; use the text-encrypt tool if you want passphrase-based encryption.

</details>

<details>
<summary>What IV or nonce size does each mode require?</summary>

CBC and CTR need a 16-byte IV, GCM needs a 12-byte nonce, and ECB takes none at all. The IV is decoded with the same **format** (hex/base64) as the key, so in hex that's 32 characters for CBC/CTR and 24 for GCM.

</details>

<details>
<summary>Where is the GCM authentication tag in the output?</summary>

For GCM the 16-byte tag is appended to the ciphertext, so the encoded output is `ciphertext ‖ tag`. When decrypting, paste that whole blob back in — if the tag doesn't verify (wrong key, wrong nonce, or tampered data), decryption fails instead of returning garbage.

</details>

<details>
<summary>Is it safe to paste a real key here?</summary>

The cipher runs entirely in your browser as WebAssembly — the key, IV and data are never transmitted anywhere. That said, ECB mode is included only for interop/teaching: it leaks patterns in the plaintext, so prefer GCM for anything real.

</details>

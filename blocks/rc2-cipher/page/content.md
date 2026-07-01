## About this tool

**RC2 cipher** encrypts or decrypts data with the **RC2 block cipher** (defined in
**RFC 2268**) in **ECB** or **CBC** mode, with hex or base64 key / IV / ciphertext.
RC2 has a 64-bit block and a separate **effective key length** (T1) that you can
configure independently of the key bytes.

> ⚠️ **RC2 is a legacy cipher and is not secure for new designs.** It survives mainly
> in old **PKCS#12** key stores, **S/MIME** messages, and some Microsoft formats. Use
> this tool only to **decrypt legacy data** or for **interop** with old systems — for
> real encryption use **AES** (the `aes-cipher` tool) or a passphrase tool.

- **Key:** 1–128 bytes (encoded with the chosen format).
- **IV:** 8 bytes (CBC only).
- **Effective key bits (T1):** 1–1024; `0` means "use the key's full bit-length."
  The same value must be set for both encrypt and decrypt.
- **Padding:** PKCS#7 to the 8-byte block. **ECB** reveals patterns — prefer **CBC**.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never leave
the device. Also available from the [gizza CLI](/) and in chat.

## FAQ

<details>
<summary>What does "effective key bits" (T1) do, and why is my decryption garbage?</summary>

RC2 deliberately limits its key schedule to T1 bits regardless of how many key
bytes you supply — a relic of 1990s export controls. If the value used to
decrypt differs from the one used to encrypt, you get garbage (or a padding
error), even with the right key bytes. Leave it at `0` to use the key's full
bit-length, or match the original system's setting — legacy software commonly
used 40 or 128.

</details>

<details>
<summary>What key and IV sizes does RC2 accept here?</summary>

The key can be **1–128 bytes**, supplied in the encoding you pick (base64 by
default, or hex). RC2's block is 64 bits, so CBC mode needs an **exactly
8-byte IV**; ECB takes no IV at all. Plaintext is padded to the 8-byte block
with PKCS#7, and the plaintext side is always treated as UTF-8 text.

</details>

<details>
<summary>Should I pick ECB or CBC mode?</summary>

CBC (the default) — ECB encrypts each 8-byte block independently, so repeated
plaintext blocks produce visibly repeated ciphertext. Choose ECB only when
the legacy format you're matching demands it, and remember decryption must
use the same mode, IV, and effective-key-bits as encryption.

</details>

<details>
<summary>Is RC2 okay to use for protecting new data?</summary>

No. RC2 is a legacy cipher that survives mainly in old PKCS#12 key stores,
S/MIME messages, and some Microsoft formats — this tool exists to decrypt
that data and interoperate with old systems. For anything new, use AES (see
the **AES cipher** tool).

</details>

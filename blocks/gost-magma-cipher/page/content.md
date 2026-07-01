## About this tool

**GOST Magma cipher** is a low-level encrypt/decrypt tool for the GOST 28147-89 /
GOST R 34.12-2015 **"Magma"** block cipher (also specified in **RFC 8891**), the
legacy 64-bit Russian standard symmetric cipher. You supply the raw **key** and
**IV**, pick the **mode**, and get the result — handy for implementing or testing
against the standard, debugging interop, or learning how the cipher works.

- **Block / key:** Magma uses a 64-bit (8-byte) block and a fixed **256-bit
  (32-byte) key**. The S-box is the standard `id-tc26-gost-28147-param-Z` set.
- **Modes:** `CBC` (default) and `ECB` (insecure, reveals patterns — included for
  completeness only). Both modes use **PKCS7** padding.
- **Encoding:** key, IV and ciphertext are **hex** or **base64**; the plaintext is
  UTF-8 text. CBC needs an 8-byte IV; ECB needs none.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

### Not sure which tool you want?

If you just want to **protect a message with a passphrase** (and have the salt,
key derivation and nonce handled safely for you), use the **text-encrypt** tool
instead — `gost-magma-cipher` is for when you already have a specific raw key, IV
and mode. For the newer 128-bit GOST cipher, see **gost-kuznyechik-cipher**.

## FAQ

<details>
<summary>What key and IV sizes does Magma require?</summary>

The key is always **32 bytes (256 bits)** — Magma has no other key size. CBC mode
additionally needs an **8-byte IV** (one 64-bit block); ECB takes no IV at all.
Supply both in the encoding you selected (hex or base64) — a 64-character hex
string or 44-character base64 string for the key.

</details>

<details>
<summary>Why won't my ciphertext decrypt?</summary>

Magma-CBC/ECB has no built-in authentication, so a wrong key, wrong IV, wrong
mode, or a hex/base64 mix-up usually surfaces as a **PKCS7 padding error** (or as
mojibake if the padding happens to parse). Double-check that all four settings
match the ones used to encrypt, including the encoding of the key and IV.

</details>

<details>
<summary>Which S-box does this implementation use?</summary>

The standard `id-tc26-gost-28147-param-Z` substitution set — the one fixed by
GOST R 34.12-2015 and RFC 8891. If you're interoperating with an old GOST
28147-89 system that used a different regional S-box, the outputs will not match.

</details>

<details>
<summary>Is ECB mode safe to use?</summary>

No — ECB encrypts every 8-byte block independently, so repeated plaintext blocks
produce repeated ciphertext blocks and patterns leak. It's included for testing
and interop against the standard only; use CBC with a random IV (or a modern AEAD
cipher) for anything real.

</details>

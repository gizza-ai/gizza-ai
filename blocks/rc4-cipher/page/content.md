## About this tool

**RC4 cipher** encrypts and decrypts text with the classic **RC4** stream cipher
(also known as ARCFOUR). You supply the **key**, optionally a **drop-N**, and pick
the **encoding** — handy for interoperating with legacy systems, solving CTFs,
testing against a spec, or learning how stream ciphers work.

- **Symmetric:** RC4 XORs your data with a key-derived keystream, so the same
  operation encrypts and decrypts. To recover a message, use the *same key, drop-N
  and encoding* you encrypted it with.
- **Key:** 1–256 bytes. Enter it as a **text** passphrase, or as an **encoded**
  (hex / base64) byte string.
- **Drop-N (RC4-drop[n]):** discards the first *n* keystream bytes (common values:
  768 or 3072) to skip RC4's statistically biased prefix. Leave it at `0` for plain
  RC4. It must match on both encrypt and decrypt.
- **Encoding:** the ciphertext (and an encoded key) are **hex** or **base64**; the
  plaintext is always UTF-8 text.

### Security warning

RC4 is **cryptographically broken** — practical attacks recover plaintext, and it
is banned from TLS. **Do not use it to protect real secrets.** This tool exists for
interop, CTFs, legacy data and education only. For real encryption use the
**aes-cipher** or **text-encrypt** tools instead.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

## FAQ

<details>
<summary>Why does decrypting give me garbage instead of an error?</summary>

RC4 is a plain XOR stream cipher with no built-in integrity check, so *any* key
"works" — a wrong key, a mismatched drop-N, or the wrong hex/base64 setting just
XORs with the wrong keystream and yields mojibake. All three settings must match
the ones used to encrypt exactly.

</details>

<details>
<summary>What does drop-N do, and what value should I use?</summary>

RC4's first keystream bytes are statistically biased, which enabled real attacks.
RC4-drop[n] discards the first *n* bytes before encrypting; the conventional
values are **768** or **3072**. It must be identical on encrypt and decrypt — a
message encrypted with drop 768 will not decrypt with drop 0. The default is 0
(plain RC4) for compatibility with legacy systems.

</details>

<details>
<summary>How do I enter a binary key?</summary>

Switch the key format from **text** to **encoded**: the key is then decoded from
hex or base64 (whichever encoding you selected for the ciphertext) instead of
being taken as a UTF-8 passphrase. Keys can be 1–256 bytes, per the RC4 spec.

</details>

<details>
<summary>Is RC4 safe to use for real data?</summary>

No. RC4 is cryptographically broken — practical attacks recover plaintext, and
it has been banned from TLS since 2015. Use this tool for interop with legacy
systems, CTFs, and learning; encrypt anything that matters with the aes-cipher
or text-encrypt tools instead.

</details>

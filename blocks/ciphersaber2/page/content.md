## About this tool

**CipherSaber-2** encrypts and decrypts text with the **CipherSaber** cipher — a
deliberately minimalist symmetric cipher built on **RC4** (ARCFOUR). You supply
the **key**, optionally the **rounds** and **encoding**, and the tool handles the
random initialization vector for you — handy for interoperating with CipherSaber
implementations, solving CTFs, or learning how stream ciphers work.

- **Symmetric:** CipherSaber XORs your data with a key-derived keystream, so the
  same operation conceptually encrypts and decrypts. To recover a message, use
  the *same key, rounds and encoding* it was encrypted with.
- **Random IV:** every encryption generates a fresh, random **10-byte
  initialization vector (IV)** and prepends it to the ciphertext in the clear.
  The decryptor reads the IV back automatically — you only need the key. You can
  optionally supply an explicit IV (encoded, 10 bytes) for deterministic output.
- **Rounds (CipherSaber-2):** the **2** variant repeats RC4's key-scheduling loop
  several times — the spec recommends **20** — to better diffuse the key and
  resist the FMS key-recovery attack that breaks CipherSaber-1. `rounds` must
  match on both encrypt and decrypt. (Set `rounds = 1` for original CipherSaber-1.)
- **Key:** enter it as a **text** passphrase, or as an **encoded** (hex / base64)
  byte string. The key plus the 10-byte IV must total at most 256 bytes.
- **Encoding:** the ciphertext (and an encoded key/IV) are **hex** or **base64**;
  the plaintext is always UTF-8 text.

### Security warning

RC4 — and therefore CipherSaber — is **cryptographically broken**; practical
attacks recover plaintext, and RC4 is banned from TLS. CipherSaber was designed
as an easy-to-memorize teaching cipher, **not** to protect real secrets. Use this
tool for interop, CTFs and education only. For real encryption use the
**aes-cipher**, **text-encrypt** or **encrypt-file** tools instead.

### Privacy

Everything runs **in your browser** via WebAssembly — your key and data never
leave the device. Also available from the [gizza CLI](/) and in chat.

### FAQ

<details>
<summary>Why is the ciphertext different every time I encrypt the same message?</summary>

Each encryption draws a fresh random 10-byte IV and prepends it to the output, so two runs never match — that's correct CipherSaber behavior. If you need reproducible output (e.g. for a test vector), supply an explicit 10-byte IV in the IV field, encoded in the selected hex/base64 format.

</details>

<details>
<summary>What rounds value do I need, and what about CipherSaber-1?</summary>

The CipherSaber-2 spec recommends **20** (the default). The value must be identical on encrypt and decrypt — a mismatch produces garbage. Set `rounds = 1` to interoperate with original CipherSaber-1 messages.

</details>

<details>
<summary>Decryption failed with "not valid UTF-8" or "ciphertext too short" — why?</summary>

"Ciphertext too short" means the decoded input has fewer than 10 bytes, so there's no room for the IV — usually a truncated paste or the wrong encoding selected. "Not valid UTF-8" means RC4 ran but produced bytes that aren't text: almost always a wrong key, wrong rounds, or a hex/base64 mismatch.

</details>

<details>
<summary>Is there a limit on the key size?</summary>

Yes — RC4's key schedule caps the session key at 256 bytes, and the 10-byte IV is part of it, so your key (passphrase bytes, or decoded bytes in encoded mode) can be at most 246 bytes. An empty key is rejected.

</details>

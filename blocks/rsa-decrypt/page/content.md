## About this tool

RSA Decrypt recovers the plaintext from a single RSA ciphertext block with the matching private key.
Use it when a message or wrapped symmetric key was encrypted with RSA-OAEP or legacy RSAES-PKCS1-v1_5
and you need to verify the payload locally. Paste a PEM private key, paste the base64 or hex
ciphertext, choose the padding and OAEP hash that the sender used, and select whether the recovered
bytes should be rendered as UTF-8 text, hex, or base64.

The private key and ciphertext are processed in WebAssembly; the tool does not upload them. OAEP
with SHA-256 is the default because it is the modern RSA encryption mode. PKCS#1 v1.5 is included
only for compatibility with old systems. RSA can decrypt only one key-sized block, so large messages
should be handled with hybrid encryption: RSA decrypts a small random content key, then a symmetric
cipher decrypts the bulk data.

### Worked example

The bundled example ciphertext was encrypted with OAEP-SHA256 to the throwaway fixture key used by
this repository's tests. Paste that fixture private key, select `padding = oaep`, `hash = sha256`,
`ciphertext_encoding = base64`, and `output_encoding = utf8`; the plaintext is `hello from
rsa-decrypt`. For binary payloads, switch `output_encoding` to `hex` or `base64` instead of trying
to force the bytes into text.

### Limits and edge cases

- The ciphertext must be exactly one RSA block: 256 bytes for a 2048-bit key, 512 bytes for a
  4096-bit key, and so on.
- OAEP decryption succeeds only when the hash setting matches the sender's OAEP/MGF1 hash.
- PKCS#8 and PKCS#1 PEM private keys are accepted. Encrypted PKCS#8 keys require a passphrase;
  legacy OpenSSL `Proc-Type: 4,ENCRYPTED` PEM keys must be converted to PKCS#8 first.
- If the decrypted bytes are not valid UTF-8, choose hex or base64 output.
- This tool does not generate keys, encrypt messages, or perform signatures. Use the neighboring RSA
  encrypt/sign/verify tools for those surfaces.

## FAQ

<details>
<summary>Why does RSA decryption fail when the private key is correct?</summary>

RSA encryption parameters must match exactly. Check the padding (`oaep` vs `pkcs1v15`), the OAEP
hash (`sha256`, `sha384`, or `sha512`), the ciphertext encoding, and whether the ciphertext was
truncated or copied with whitespace changes. A ciphertext encrypted to a different public key cannot
be decrypted by this private key.

</details>

<details>
<summary>Can I decrypt a whole file with this?</summary>

Not directly. RSA encryption is for small payloads such as a random AES key or short secret. A file
should be encrypted with a symmetric cipher, with RSA used only to unwrap the symmetric key. If you
paste a full file ciphertext here it will usually be much longer than the RSA key size and the tool
will reject it.

</details>

<details>
<summary>Which padding should I choose?</summary>

Use OAEP when you control both sides or know the sender used a modern default. Choose PKCS#1 v1.5
only for compatibility with older systems that explicitly say they used RSAES-PKCS1-v1_5. The
selected padding must match the original encryption mode; there is no safe auto-detection.

</details>

<details>
<summary>Is it safe to paste a private key here?</summary>

The decryption runs locally in WebAssembly and the key is not uploaded by this page. You should still
use a throwaway key for tests when possible, avoid pasting production keys on shared machines, and
clear the tab after use.

</details>

## About this tool

RNCryptor is a container format used by mobile and scripting libraries to wrap password-encrypted data. This tool emits the full password-based v3 blob: version `0x03`, options `0x01`, an 8-byte encryption salt, an 8-byte HMAC salt, a 16-byte AES-CBC IV, padded ciphertext, and a trailing HMAC-SHA256 tag. The result is not bare AES ciphertext; it is the interoperable container another RNCryptor implementation expects.

The cryptographic settings are fixed by the v3 password format: AES-256-CBC with PKCS#7 padding, PBKDF2-HMAC-SHA1 with 10,000 iterations for each 32-byte key, and HMAC-SHA256 over the header plus ciphertext. Leave the salt and IV fields empty for real data so the browser generates fresh random bytes. Fill them only when you need byte-for-byte reproducible output for a test vector.

Worked example: set operation to `encrypt`, data to `01`, password to `thepassword`, input encoding to `hex`, output encoding to `hex`, encryption salt to `0001020304050607`, HMAC salt to `0102030405060708`, and IV to `02030405060708090a0b0c0d0e0f0001`. The output starts with `030100010203040506070102030405060708020304...`; switching operation to `decrypt` with that container returns `01`.

Limits and edge cases: decoded inputs are capped at 4 MiB to keep browser memory bounded; only RNCryptor version 3 password containers are supported; key-based containers and legacy v1/v2 blobs are rejected; wrong passwords and tampered containers fail during HMAC verification before any plaintext is returned.

## FAQ

<details>
<summary>Why are there no controls for AES mode, key size, or PBKDF2 iterations?</summary>

Those values are part of the RNCryptor v3 password format. Changing them would create a private blob that looks encrypted but cannot be opened by standard RNCryptor libraries, so this tool keeps them pinned and exposes only byte-handling controls.

</details>

<details>
<summary>Should I fill in the salt and IV fields?</summary>

Usually no. Empty fields make the browser generate fresh random salts and a fresh IV for each encryption, which is the safe default. The hex override fields exist for deterministic tests, migrations, and spec-vector reproduction.

</details>

<details>
<summary>Can this decrypt containers from an iOS or Android app?</summary>

Yes, if they are password-based RNCryptor v3 containers. Paste the base64 or hex container, choose `decrypt`, and enter the same password. The HMAC is checked before unpadding, so a wrong password or modified blob returns an error rather than confusing plaintext.

</details>

<details>
<summary>What input encoding should I choose for binary data?</summary>

Use `hex` or `base64` so the data is decoded to raw bytes before encryption. `text` treats the input as UTF-8 text when encrypting; when decrypting, `text` auto-detects whether the pasted container looks like hex or base64.

</details>

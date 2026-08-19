## About this tool

Encrypt small text snippets into an ASCII-armored age file. The output starts with
`-----BEGIN AGE ENCRYPTED FILE-----`, so it can be copied into chat, tickets, or a
terminal and later decrypted by compatible age clients.

Use passphrase mode when the recipient already knows a shared secret. Use
recipients mode when you have one or more native age X25519 public keys beginning
with `age1`. Recipient mode accepts keys on separate lines, spaces, commas, or
semicolons, and ignores `#` comments in pasted recipient files.

Example passphrase run:

1. Paste `Deploy key rotates at 17:00 UTC.` as the plaintext.
2. Leave mode set to `passphrase`.
3. Enter a strong passphrase and keep the work factor at `14`.
4. Copy the armored age ciphertext from the result.

Limits and edge cases: this page encrypts text up to 1 MiB. Passphrase work
factor is capped at 15 because higher scrypt settings exceed the wasm memory
sandbox. The tool does not decrypt, generate identities, encrypt files, or accept
SSH recipients; use the age CLI for those workflows.

## FAQ

<details>
<summary>Can I decrypt the result on this page?</summary>

No. This tool only encrypts plaintext to age ciphertext. Decrypt with a compatible
age client using the same passphrase or a matching `AGE-SECRET-KEY-1...` identity.

</details>

<details>
<summary>What kind of recipient key does this accept?</summary>

Recipient mode accepts native age X25519 public recipients that start with
`age1`. It rejects private identities and SSH public keys so they are not pasted
into the wrong field by mistake.

</details>

<details>
<summary>Why is the work factor limited to 15?</summary>

The work factor controls scrypt memory use in passphrase mode. Higher values can
be useful on a desktop age CLI, but they exceed the memory available to this wasm
tool. The range 10-15 keeps encryption and later decryption practical here.

</details>

<details>
<summary>Why does the same input produce different ciphertext each time?</summary>

Age encryption uses fresh randomness for every file. Different ciphertext for the
same plaintext and passphrase is expected and prevents repeated messages from
looking identical.

</details>

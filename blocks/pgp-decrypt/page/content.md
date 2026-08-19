## About this tool

PGP decrypt opens an **ASCII-armored OpenPGP (PGP/GPG) message** and shows the decrypted payload as text, base64, or hex. Paste the full `-----BEGIN PGP MESSAGE-----` block, then provide either the recipient private key (for public-key encrypted mail) or the password used for a symmetric `gpg --symmetric` message. Everything runs locally in WebAssembly, so messages, private keys, and passphrases never leave your browser.

### What it handles

- **Public-key encrypted messages** — paste the matching `-----BEGIN PGP PRIVATE KEY BLOCK-----` key. If that key is passphrase-protected, enter the passphrase too.
- **Password-encrypted messages** — leave the private key blank and enter the message password in the passphrase field.
- **Compressed payloads** — common OpenPGP compression layers are decompressed before output.
- **Encrypted-and-signed messages** — signature metadata is reported. Paste the signer's public key to verify the signature instead of only seeing that one was present.
- **Binary payloads** — `auto` shows UTF-8 text when possible and falls back to base64 for binary bytes. Choose `base64` or `hex` when you want an explicit encoding.

### Worked example

1. Paste a complete armored encrypted message in **Encrypted message**.
2. Paste the matching armored private key in **Your private key**.
3. If the private key is protected, enter its passphrase. For a symmetric message, leave the private key blank and enter the message password instead.
4. Leave **Show the plaintext as** set to **Auto** for normal text messages.

A successful text message returns JSON like:

```json
{
  "plaintext": "attack at dawn",
  "encryption": "public-key",
  "output_format": "text",
  "bytes": 14,
  "binary": false,
  "compressed": false
}
```

For signed messages, the result also includes a `signature` object. `valid: true` means the supplied public key verified the embedded signature; `valid: null` means the message was signed but no public key was supplied.

### Limits and edge cases

- The armored input is capped at **4 MiB** to keep browser memory usage predictable.
- The tool accepts armored OpenPGP messages, not raw `.gpg` binary files. Armor binary files first or decrypt them with a desktop OpenPGP client.
- A valid signature proves the message matches the supplied signing key; it does not prove you trust that key. Compare fingerprints through a trusted channel.
- If the private key is wrong, the error includes the recipient key ID when the message exposes one.

## FAQ

<details>
<summary>Do I need a private key, a passphrase, or both?</summary>

For a message encrypted to your public key, paste the matching private key. If that private key is protected, enter its passphrase too. For a symmetric `gpg --symmetric` message, leave the private key blank and enter the message password in the passphrase field.

</details>

<details>
<summary>Can this decrypt binary files?</summary>

It can decrypt binary payloads inside an armored OpenPGP message, but the page returns them as text encodings. Use `auto` to fall back to base64 when bytes are not UTF-8, or choose `base64` or `hex` explicitly. Very large encrypted files are better handled by a desktop OpenPGP client.

</details>

<details>
<summary>Why does it say the private key is not a recipient?</summary>

OpenPGP messages are encrypted to one or more recipient key IDs. That error means the pasted private key does not match any recipient session key in the message, or the message uses a hidden recipient and this key still could not unlock it. Check that you pasted the private key for the address or fingerprint the sender used.

</details>

<details>
<summary>Does this verify encrypted signatures automatically?</summary>

The tool reports when a decrypted message contains a signature. To actually verify it, paste the signer's public key in the optional public-key field. Without that key, the result marks the signature as present but unverified.

</details>

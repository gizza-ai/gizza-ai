## About this tool

PGP verify checks an **OpenPGP (PGP/GPG) signature** against a message and the
signer's public key, confirming that the message was signed by the holder of
that key and has not been altered since. Everything runs locally in your
browser through WebAssembly — your message and keys never leave your device and
are never uploaded to a server.

### What it does

- **Detached signatures** — paste a `-----BEGIN PGP SIGNATURE-----` block in the
  signature field and the exact original text in the message field. This is what
  `gpg --detach-sign` and most release-signing workflows produce.
- **Clearsigned messages** — paste a `-----BEGIN PGP SIGNED MESSAGE-----` block.
  The signed text is embedded in the block, so the message field is ignored.
- The signature shape is **auto-detected**, so you don't have to choose a mode.
- Verification tries the public key's **primary key and every subkey**, so a
  message signed with a signing subkey still verifies.

### What you get back

A structured result reporting:

- **valid** — `true` only when the signature checks out against the key.
- **signer_key_id** and **signer_fingerprint** — which key produced the
  signature, so you can confirm it is the key you expect.
- **signer_user_id** — the key's primary User ID (name and email), so you can
  see who the key claims to belong to.
- **signed_at** — when the signature was created.
- **hash_algorithm** — the hash used by the signature (for example, SHA256).
- **signed_text** — for a clearsigned block, the exact text that was signed.
- **error** — a plain-English reason when verification fails (for example, the
  message was altered, or it was signed by a different key).

Note that a valid signature confirms the message was signed by the holder of
that key — it does **not** by itself establish that you trust the key's owner.
Always check the fingerprint against one you obtained from a trusted channel.

### Tips

- For a detached signature, the message must be the **exact, unmodified bytes**
  that were signed — even a trailing newline difference will make it fail.
- This tool verifies signatures; to create one use the **PGP sign** tool, and to
  generate a key use **Generate PGP key pair**.

## FAQ

<details>
<summary>Do I have to tell the tool whether my signature is detached or clearsigned?</summary>

No — the shape is auto-detected from the armor. A
`-----BEGIN PGP SIGNATURE-----` block is treated as detached (so the message
field must hold the original text), while a
`-----BEGIN PGP SIGNED MESSAGE-----` block carries its own text and the
message field is ignored. Anything else is rejected with a "no PGP signature
found" error.

</details>

<details>
<summary>Why does verification fail when my message looks identical to the signed one?</summary>

A detached signature covers the **exact bytes** that were signed. A trailing
newline you didn't notice, CRLF vs LF line endings, or an editor stripping
whitespace is enough to flip `valid` to `false` even though the text *looks*
the same. Copy the original file's contents unmodified.

</details>

<details>
<summary>The message was signed with a signing subkey — will it still verify?</summary>

Yes. Verification is attempted against the public key's primary key **and
every subkey**, so the common GPG setup of certify-only primary + signing
subkey works without any extra steps. The result tells you which key ID and
fingerprint actually produced the signature.

</details>

<details>
<summary>Does valid: true mean I can trust the sender?</summary>

Not by itself. It proves the message was signed by the holder of *that key*
and wasn't altered since. Whether the key really belongs to the person you
think it does is a separate question — compare the reported fingerprint
against one you obtained through a trusted channel.

</details>

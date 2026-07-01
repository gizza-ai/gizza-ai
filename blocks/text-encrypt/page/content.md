## About this tool

**Text encrypt** locks a piece of text behind a passphrase using **AES-256-GCM**
authenticated encryption, and gives you back a compact **base64 token** you can
paste into an email, a note, or a chat. Anyone with the token *and* the passphrase
can decrypt it; without the passphrase it's unreadable.

- **Encrypt:** paste text + a passphrase → get a token.
- **Decrypt:** paste the token + the same passphrase → get the text back.

### How it works

The passphrase is stretched into a 256-bit key with **PBKDF2-HMAC-SHA256**
(200,000 iterations). Each encryption uses a fresh random **salt** and **nonce**,
so encrypting the same text twice produces different tokens. The token is
self-describing (`salt | nonce | ciphertext+tag`), so only the passphrase is
needed to decrypt. A wrong passphrase or a tampered token **fails cleanly** —
the GCM authentication tag won't verify.

### Privacy

Everything runs **in your browser** via WebAssembly — the text and passphrase are
never uploaded to a server. You can also run it from the [gizza CLI](/) or inside
a gizza chat.

### Notes

Your security depends on the strength of your passphrase — use a long, unique one.
To encrypt whole files instead of text, see the file-encryption tool.

## FAQ

<details>
<summary>Why do I get a different token each time I encrypt the same text?</summary>

That's intentional: every encryption draws a fresh random salt and nonce, so
identical inputs produce different tokens. All of them decrypt to the same
text with the same passphrase — and the randomness prevents anyone spotting
that two tokens hide the same message.

</details>

<details>
<summary>I lost the passphrase — can the text be recovered?</summary>

No. The 256-bit key exists only while your passphrase is being used; nothing
is stored anywhere, and there is no reset or backdoor. Without the exact
passphrase, the GCM authentication check fails and decryption returns an
error, not partial text.

</details>

<details>
<summary>Can I decrypt a token on another device, or does it expire?</summary>

Tokens never expire and aren't tied to a device. The token embeds its own salt
and nonce (`salt | nonce | ciphertext+tag`), so this page, the gizza CLI, or a
gizza chat can decrypt it anywhere — just note it's this tool's format, not a
generic OpenSSL container.

</details>

<details>
<summary>Does the 200,000-iteration key stretching make a weak passphrase safe?</summary>

It helps — PBKDF2-HMAC-SHA256 at 200,000 iterations makes each guess much more
expensive — but it can't rescue "hunter2". A short or common passphrase is
still brute-forceable offline; a long, unique phrase is what actually protects
the token.

</details>

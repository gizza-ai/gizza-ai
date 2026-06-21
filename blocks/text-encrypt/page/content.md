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

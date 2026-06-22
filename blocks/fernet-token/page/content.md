## What is a Fernet token?

Fernet is a simple, opinionated recipe for **symmetric (secret-key) authenticated
encryption**. A single 32-byte key both encrypts and signs your data, and the
resulting token is a compact, url-safe string you can drop into a URL, a cookie, a
config file, or a database column. Fernet guarantees that a token created with the
key cannot be read or undetectably altered without it.

Under the hood a Fernet token combines:

- **AES-128 in CBC mode** with a random IV for confidentiality,
- **HMAC-SHA256** over the whole token for integrity and authenticity, and
- an embedded **timestamp** so you can enforce a time-to-live (TTL) and reject
  tokens that are too old.

This tool implements that recipe exactly, entirely in your browser via WebAssembly.

## How to use it

**Encrypt:** choose *encrypt*, paste your text, and leave the key blank to generate a
fresh 32-byte key — the output includes both the token and the key. Save the key
somewhere safe; it is the only thing that can read the token back. To reuse an
existing key, paste it into the key field.

**Decrypt:** choose *decrypt*, paste the token and the key it was created with. The
tool verifies the HMAC, decrypts the text, and reports the token's creation time. Set
a **TTL** (in seconds) to reject tokens older than that age — leave it at `0` to skip
the age check.

## What a Fernet key looks like

A Fernet key is the url-safe base64 encoding of 32 random bytes, for example
`cw_0x689RpI-jtRR7oE8h_eQsKImvJapLeSbXpwF4e4=`. The first 16 bytes are the signing
key (HMAC) and the last 16 bytes are the encryption key (AES). This is exactly the
format used by Python's `cryptography` `Fernet` class and compatible libraries, so
tokens created here interoperate with them.

## Privacy

Everything happens locally in your browser through WebAssembly. Your text, your key,
and the tokens are never uploaded, logged, or sent to any server. You can disconnect
from the network and the tool still works.

## When to use Fernet

Fernet is a good fit for short-lived, self-contained secrets: signed session tokens,
password-reset links, single-use download URLs, encrypted cookies, and small
configuration secrets. Because it is symmetric, the same party (or trusted parties
sharing the key) both creates and reads the tokens. For asymmetric needs — where the
sender and reader hold different keys — use a public-key scheme such as PGP instead.

## FAQ

**Is this compatible with Python's `cryptography` Fernet?** Yes. The token format,
key format, and TTL semantics follow the published Fernet spec, so a token made here
can be read by `Fernet(key).decrypt(token)` and vice versa.

**What does TTL do?** On decrypt, TTL is the maximum allowed age of a token in
seconds. If the token's embedded timestamp is older than `now - ttl` (or set in the
future), decryption is refused. A TTL of `0` disables the check.

**What happens with the wrong key or a tampered token?** Verification fails before any
plaintext is produced — you get a clean error, never garbage output.

**Can I rotate keys?** Generate a new key for new tokens and keep old keys around long
enough to read tokens that are still within their TTL. This tool reads one key at a
time.

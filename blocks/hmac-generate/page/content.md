## About this tool

This HMAC generator computes a **keyed-hash message authentication code** from a
message and a secret key, right in your browser. The computation runs locally in
WebAssembly — your message and key are **never uploaded** to a server, which
makes it safe for API secrets, signing keys, and other sensitive values.

An HMAC (RFC 2104) combines a secret key with a message and a cryptographic hash
to produce a short tag. Unlike a plain hash, the tag **cannot be recomputed or
forged without the key**, so HMAC proves both that a message is unaltered
(integrity) and that it came from someone holding the key (authenticity). It is
the mechanism behind API request signing, JWT `HS256` signatures, and webhook
verification for services like Stripe and GitHub.

### Supported algorithms

- **HMAC-SHA256** (the default) — the standard modern choice, used by AWS
  Signature v4, JWT `HS256`, and most webhook signatures.
- **SHA-2 family** — **HMAC-SHA224, HMAC-SHA384, HMAC-SHA512** for longer tags.
- **SHA-3 family** — **HMAC-SHA3-256** and **HMAC-SHA3-512**, the Keccak-based
  NIST standard.
- **HMAC-SHA1** and **HMAC-MD5** — legacy, but still required by some older APIs
  (e.g. OAuth 1.0a `HMAC-SHA1`). HMAC remains secure even with these weaker
  hashes for many uses, but prefer SHA-256 for new designs.

### Options

- **Algorithm** — choose the underlying hash.
- **Interpret message as** / **Interpret key as** — read each input as plain
  UTF-8 text (default), or decode it from **hex** or **base64** first. Use
  hex/base64 for binary keys or pre-encoded payloads.
- **Output format** — return the tag as **hex** (default) or **base64**.
- **Uppercase hex** — emit the hex tag in uppercase.

### Notes

- HMAC is a one-way function: the tag cannot be reversed back into the message
  or the key.
- To verify a signature, recompute the HMAC over the same message and key and
  compare it (ideally with a constant-time comparison) against the expected tag.
- For an **unkeyed** hash (no secret key), use the text hash generator instead.

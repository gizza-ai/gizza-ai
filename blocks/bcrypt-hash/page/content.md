## About this tool

**Bcrypt hash** hashes a password with **bcrypt** — the long-standing, adaptive
password-hashing algorithm derived from the Blowfish cipher and still widely used
across web frameworks and databases. The result is a standard **modular-crypt
string** that embeds the variant, cost, salt and hash, e.g.:

```
$2b$12$<22-char-salt><31-char-hash>
```

- **Hash** mode generates a fresh random salt and returns the hash string. Tune the
  **cost** (work factor, 4–31; default 12). Each step up *doubles* the time it takes
  to compute — and to crack.
- **Verify** mode checks a password against a bcrypt hash you paste. The variant and
  cost are read from the string itself, so it accepts `$2a$`, `$2b$`, `$2x$` and the
  PHP-style `$2y$` hashes.

### Privacy

Everything runs **in your browser** via WebAssembly — your password is **never
uploaded** to a server. Also available from the [gizza CLI](/) and in chat.

### Notes

bcrypt only considers the **first 72 bytes** of a password; longer inputs are
rejected here so you don't silently lose data. For new systems, Argon2id or scrypt
are generally preferred — but bcrypt remains a solid, battle-tested choice, and you
often need it to match an existing hash.

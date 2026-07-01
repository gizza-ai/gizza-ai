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

## FAQ

<details>
<summary>Why does the same password give a different hash every time?</summary>

Because each hash embeds a fresh random salt — that's how bcrypt is supposed to work.
Two hashes of the same password will never match character-for-character. To check a
password against an existing hash, use **verify** mode, which reads the salt and cost
out of the hash string and recomputes it.

</details>

<details>
<summary>What cost (work factor) should I pick?</summary>

The cost can be 4–31 and defaults to **12**. Each +1 doubles the computation time —
for hashing and for an attacker cracking it. 10–12 is typical for web logins today;
values above ~15 can take seconds per hash in the browser, so raise it gradually.

</details>

<details>
<summary>Why is my long password rejected?</summary>

bcrypt only hashes the **first 72 bytes** of input. Rather than silently truncating
(which some libraries do), this tool returns an error for anything longer than 72
bytes so you know exactly what was hashed. If you need longer secrets, pre-hash them
(e.g. SHA-256) before feeding bcrypt.

</details>

<details>
<summary>Can it verify $2y$ hashes from PHP?</summary>

Yes. Verify mode accepts the `$2a$`, `$2b$`, `$2x$` and PHP-style `$2y$` variant tags —
they all describe the same underlying algorithm, so a `password_hash()` string from PHP
verifies fine here.

</details>

## About this tool

**Argon2 hash** hashes a password with **Argon2id** — the winner of the Password
Hashing Competition and the modern, memory-hard algorithm recommended for storing
passwords. The result is a standard **PHC string** that embeds the algorithm,
parameters, salt and hash, e.g.:

```
$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
```

- **Hash** mode generates a fresh random salt and returns the PHC string. Tune the
  **memory** (KiB), **iterations**, and **parallelism** — the defaults
  (19 MiB / 2 / 1) follow OWASP guidance.
- **Verify** mode checks a password against a PHC string you paste (the parameters
  are read from the string itself).

### Privacy

Everything runs **in your browser** via WebAssembly — your password is **never
uploaded** to a server. Also available from the [gizza CLI](/) and in chat.

### Why Argon2id?

Unlike fast hashes (MD5/SHA), Argon2id is deliberately **slow and memory-hard**,
which makes large-scale password cracking expensive. Use it (or bcrypt/scrypt) for
storing user passwords — never a plain SHA.

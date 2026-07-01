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

## FAQ

<details>
<summary>Why do I get a different hash every time for the same password?</summary>

That's by design. Hash mode generates a fresh random 16-byte salt on every run, and
the salt is part of the PHC output — so two hashes of the same password never match.
To check a password against an existing hash, use **verify** mode instead of
comparing strings.

</details>

<details>
<summary>How do I verify a password against an existing Argon2 hash?</summary>

Switch the mode to **verify**, enter the password, and paste the full PHC string
(everything from `$argon2id$…` onward) into the hash field. The memory, iteration,
and parallelism parameters are read from the string itself, so you don't need to
know what settings produced it. Hashes made with the `argon2i` or `argon2d` variants
verify too — the algorithm is taken from the PHC string.

</details>

<details>
<summary>What parameter ranges does this tool accept?</summary>

Memory cost from 8 KiB up to 1,048,576 KiB (1 GiB), iterations 1–50, and parallelism
1–16. The defaults — 19,456 KiB (19 MiB), 2 iterations, 1 lane — follow the current
OWASP recommendation for Argon2id. Raising memory is generally the most effective
way to make cracking more expensive.

</details>

<details>
<summary>Is it safe to type a real password here?</summary>

The hash and verify computations run entirely in your browser via WebAssembly;
the password is never sent to a server. That said, for production systems you
should hash passwords server-side at the point of storage — this tool is for
testing, debugging, and generating hashes you control.

</details>

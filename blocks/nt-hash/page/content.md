## About this tool

This NT hash generator computes the **NT (NTLM) hash** of a password right in
your browser. The hashing runs locally in WebAssembly — your input is **never
uploaded** to a server, which makes it safe for real passwords and other
sensitive strings.

The NT hash — also called the **NTLM hash** or **NTOWF** — is defined as
`MD4(UTF-16LE(password))`: the password is encoded as little-endian UTF-16, then
hashed with MD4. The result is a fixed 128-bit (16-byte) digest, shown as 32
hexadecimal characters. It is the value stored in the Windows **SAM** and
**NTDS.dit** databases and used by **NTLM authentication** and pass-the-hash, so
a quick NT hash is handy when matching or verifying those values during a
password audit or CTF.

### Options

- **Output format** — return the digest as 32-character **hex** (default, the
  conventional NTLM form) or as **base64** (24 characters).
- **Uppercase hex** — emit the hex digest in uppercase.

### Notes

- **The NT hash is unsalted.** The same password always produces the same hash,
  so identical passwords share a hash and the value is directly vulnerable to
  rainbow-table and pass-the-hash attacks.
- **MD4 is cryptographically broken and very fast.** The NT hash therefore
  offers essentially **no protection** against offline cracking — it exists for
  Windows/NTLM interop, not for security.
- **Do not use the NT hash to store new passwords.** For securely storing
  passwords, use a slow, salted algorithm — try the argon2-hash or bcrypt-hash
  tool.
- The NT hash is a one-way function: a digest cannot be reversed back into the
  original password.
- To hash text with a modern general-purpose algorithm instead, use the
  sha256-hash tool.

## FAQ

<details>
<summary>What is the NT hash of an empty password?</summary>

`31d6cfe0d16ae931b73c59d7e0c089c0` — the well-known MD4 of an empty UTF-16LE
string. If you leave the password field blank you'll get exactly this value,
which is a handy sanity check that the tool is working.

</details>

<details>
<summary>Why must the password be UTF-16LE and not UTF-8?</summary>

The NTLM spec defines the NT hash as `MD4(UTF-16LE(password))`, so the tool
encodes each character as a little-endian UTF-16 code unit before hashing.
That's why non-ASCII passwords hash to the same value Windows stores — hashing
the UTF-8 bytes would give a different, incompatible digest.

</details>

<details>
<summary>Does the uppercase option change the actual hash?</summary>

No — it only changes how the same 16-byte digest is *displayed*. Uppercase
affects the hex form only; base64 output ignores it. Whether you show
`8846F7...` or `8846f7...`, it's the identical NT hash.

</details>

<details>
<summary>Should I use the NT hash to store passwords in my app?</summary>

No. The NT hash is unsalted and MD4 is fast and broken, so it offers no real
protection against offline cracking — it exists purely for Windows/NTLM
interop and audits. For storing new passwords use a slow, salted algorithm
like argon2 or bcrypt.

</details>

## FAQ

<details>
<summary>Why doesn't the result match MD4 of my password from other tools?</summary>

Because the NT hash is **not** MD4 of the raw bytes — the password is first
encoded as **UTF-16LE** (every ASCII character becomes two bytes), and *that*
byte string is MD4-hashed. A generic MD4 tool hashing UTF-8/ASCII input will
give a different digest for the same password. Command-line pipelines often
also sneak in a trailing newline, which changes the hash again

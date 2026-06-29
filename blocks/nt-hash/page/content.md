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

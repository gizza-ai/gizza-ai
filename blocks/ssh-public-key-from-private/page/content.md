## About this tool

Every SSH key pair has a **private** key and a matching **public** key. The
public key is fully determined by the private key — it can always be
re-derived from it. This tool reads your private key and writes out the
corresponding **OpenSSH public key line**: the single line you put in
`~/.ssh/authorized_keys`, paste into GitHub/GitLab, or save as `id_*.pub`.

It is the offline, in-browser equivalent of `ssh-keygen -y -f id_key`.

### Output format

The result is one line in the standard OpenSSH wire format:

```
<type> <base64-key-blob> [comment]
```

for example `ssh-ed25519 AAAAC3Nza... you@laptop`. The type prefix is one of
`ssh-rsa`, `ecdsa-sha2-nistp256`, `ecdsa-sha2-nistp384` or `ssh-ed25519`,
matching the algorithm of your key.

### Supported keys

- **RSA** → `ssh-rsa`. PKCS#8 (`-----BEGIN PRIVATE KEY-----`) or traditional
  PKCS#1 (`-----BEGIN RSA PRIVATE KEY-----`).
- **ECDSA** → `ecdsa-sha2-nistp256` / `ecdsa-sha2-nistp384`. NIST **P-256** and
  **P-384**, in PKCS#8 or SEC1 (`-----BEGIN EC PRIVATE KEY-----`) form.
- **Ed25519** → `ssh-ed25519`. PKCS#8.

You can also paste the raw DER bytes of the private key as hex or base64 and
pick the matching format.

### A note on OpenSSH-format keys

Modern `ssh-keygen` writes private keys in its own
`-----BEGIN OPENSSH PRIVATE KEY-----` container, which this tool does **not**
read. Convert such a key to PEM first and then paste it here:

```
ssh-keygen -p -m PEM -f id_key
```

### Privacy

Everything runs in WebAssembly inside your browser. The private key you paste
is never uploaded to any server.

## FAQ

<details>
<summary>My key starts with "BEGIN OPENSSH PRIVATE KEY" — why won't it parse?</summary>

That's the modern OpenSSH-proprietary container, which this tool doesn't read.
Convert the key to PEM in place first — `ssh-keygen -p -m PEM -f id_key`
(re-enter your passphrase, or press Enter twice for none) — then paste the
resulting `BEGIN RSA/EC PRIVATE KEY` block. Ed25519 keys converted this way work
too.

</details>

<details>
<summary>Which algorithms and formats does it accept?</summary>

RSA (PKCS#8 or PKCS#1 PEM) → `ssh-rsa`; ECDSA on P-256/P-384 (PKCS#8 or SEC1
PEM) → `ecdsa-sha2-nistp256/384`; Ed25519 (PKCS#8) → `ssh-ed25519`. You can
also paste the key's raw DER bytes as hex or base64 — set the key type
explicitly (`rsa`/`ec`/`ed25519`) if `auto` can't tell from raw DER.

</details>

<details>
<summary>How do I get the "user@host" part on the end of the line?</summary>

Fill in the **comment** field — it's appended verbatim after the base64 blob,
exactly like the comment `ssh-keygen` embeds. It's purely a label: servers
ignore it, so leaving it blank produces an equally valid `authorized_keys`
line.

</details>

<details>
<summary>Can I safely paste a private key into a web page?</summary>

The derivation runs entirely inside your browser via WebAssembly — the key is
never transmitted, and only the public line is output. For production or
high-value keys, though, prefer the offline equivalent `ssh-keygen -y -f
id_key`; use this page when a terminal isn't handy or for dev/test keys.

</details>

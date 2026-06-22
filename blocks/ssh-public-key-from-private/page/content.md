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

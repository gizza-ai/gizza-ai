## About this tool

Every asymmetric key pair has a **private** key and a matching **public** key.
The public key can always be re-derived from the private key — it is, in fact,
embedded inside it. This tool reads your private key and writes out the
corresponding public key as a standard PEM **SubjectPublicKeyInfo** block
(`-----BEGIN PUBLIC KEY-----`), the form every library and service accepts.

It is the offline, in-browser equivalent of `openssl pkey -in key.pem -pubout`.

### Supported keys

- **RSA** — PKCS#8 (`-----BEGIN PRIVATE KEY-----`) or traditional PKCS#1
  (`-----BEGIN RSA PRIVATE KEY-----`).
- **EC** — NIST **P-256** and **P-384**, in PKCS#8 or SEC1
  (`-----BEGIN EC PRIVATE KEY-----`) form.
- **Ed25519** — PKCS#8.

### How it works

1. Paste a **private** key as PEM, or paste raw DER bytes as **hex** or
   **base64**.
2. Leave **key type** on `auto` — it detects the algorithm from the PEM label
   and otherwise tries each one. Set it explicitly only to disambiguate raw DER.
3. The matching public key is printed as a `-----BEGIN PUBLIC KEY-----` PEM
   block.

### Privacy

Everything runs locally in your browser via WebAssembly. Your **private key is
never uploaded** anywhere — which is exactly what you want when handling secret
key material.

### Tips

- The output is always the universal **SPKI / PKCS#8** public-key form, so it
  drops straight into OpenSSL, OpenSSH (after `ssh-keygen -i`), JWT libraries,
  and TLS configs.
- Raw DER hex input accepts `0x` prefixes and `:` / `-` / whitespace
  separators.
- Only the public half is exposed — this tool never emits any private material.

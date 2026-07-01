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

## FAQ

<details>
<summary>Which private-key formats can I paste?</summary>

RSA in PKCS#8 (`BEGIN PRIVATE KEY`) or PKCS#1 (`BEGIN RSA PRIVATE KEY`), EC
P-256/P-384 in PKCS#8 or SEC1 (`BEGIN EC PRIVATE KEY`), and Ed25519 in PKCS#8.
You can also paste raw DER bytes as hex (with `0x`, `:`, `-`, or whitespace
separators) or base64 — set the key type explicitly if auto-detection can't tell
which algorithm raw DER belongs to.

</details>

<details>
<summary>Why does my EC key give "only these NIST curves are supported"?</summary>

The EC path tries P-256 and P-384 only. Keys on other curves — P-521,
secp256k1 (Bitcoin/Ethereum), brainpool — will not parse. For those, derive the
public key with your usual library or `openssl pkey -pubout` instead.

</details>

<details>
<summary>Does it work on passphrase-protected private keys?</summary>

No — an encrypted PEM (`ENCRYPTED PRIVATE KEY`, or a legacy `Proc-Type:
4,ENCRYPTED` block) can't be parsed without the passphrase and is rejected.
Decrypt it locally first (`openssl pkey -in enc.pem -out plain.pem`), then paste
the decrypted key.

</details>

<details>
<summary>Is pasting a private key into a web page really safe?</summary>

This page performs the derivation entirely inside your browser via WebAssembly —
the key is never transmitted, and only the public half is ever printed. That
said, the standing advice for high-value keys applies: prefer an offline
terminal (`openssl pkey -pubout`) for production secrets, and treat this tool as
the convenient equivalent for dev and test keys.

</details>

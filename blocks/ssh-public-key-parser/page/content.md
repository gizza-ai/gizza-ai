## About this tool

**SSH public key parser** takes an OpenSSH **public** key and tells you what is actually
inside it: the algorithm, the key family and size in bits, the comment, and the
fingerprints — the same `SHA256:` and `MD5:` values `ssh-keygen -l` prints. It is the
answer to "someone pasted a key into a ticket, is this the one I authorised?" without
having to save the key to a file and shell out.

Everything runs **in your browser** via WebAssembly. Nothing is uploaded.

### What you can paste

- **A plain `id_*.pub` line** — `ssh-ed25519 AAAAC3Nza… alice@example.com`.
- **An `authorized_keys` line with an options prefix** — `command="…",no-pty,no-agent-forwarding ssh-rsa AAAA… ops@host`.
  The options are parsed out and listed separately.
- **A `known_hosts` entry** — host patterns or a `|1|` HMAC-hashed host, optionally
  prefixed with `@cert-authority` or `@revoked`.
- **An OpenSSH certificate** — any `*-cert-v01@openssh.com` blob.
- **An RFC 4716 block** — the `---- BEGIN SSH2 PUBLIC KEY ----` form that PuTTY and
  several commercial SSH servers export.

Multiple keys at once are fine: one per line, blank lines and `#` comments ignored.

### Worked example

Paste an Ed25519 key:

```
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPc21YeL9wdmn0Bvy1dVCZH/rO/hcbVFBt5YQ/Y8+oOy alice@example.com
```

and you get back:

```json
{
  "key_count": 1,
  "unique_fingerprints": 1,
  "keys": [
    {
      "index": 1,
      "source_format": "openssh",
      "algorithm": "ssh-ed25519",
      "key_type": "Ed25519",
      "is_certificate": false,
      "key_size_bits": 256,
      "comment": "alice@example.com",
      "fingerprint_sha256": "SHA256:/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4",
      "fingerprint_md5": "MD5:1e:e5:90:86:13:ab:0e:5a:24:3a:30:5d:7b:53:e3:fe",
      "blob_bytes": 51,
      "strength": "strong",
      "warnings": []
    }
  ]
}
```

`unique_fingerprints` is worth a glance when you paste a whole `authorized_keys` file —
if it's lower than `key_count`, the same key is authorised twice under different
comments.

### Verifying a fingerprint

Put a fingerprint you were given out of band into **Expected fingerprint** and every key
gains a `fingerprint_match` boolean, plus the report gains
`expected_fingerprint_matched`. The comparison is deliberately forgiving about how the
fingerprint was written down: `SHA256:<base64>`, the bare base64, `MD5:aa:bb:…`, bare
colon-hex and plain hex all work, and prefixes, colons, spaces and hex case are ignored.
That means you can paste straight out of a wiki page, a `ssh` host-key prompt, or a cloud
console without reformatting it.

### Strength ratings and warnings

Each key is rated `strong`, `acceptable` or `weak`, with plain-language warnings:

- **DSA (`ssh-dss`)** — obsolete; OpenSSH disabled it by default in 7.0 and removed it
  entirely in 9.8.
- **RSA under 2048 bits** — below the minimum OpenSSH accepts by default.
- **RSA under 3072 bits** — accepted, but 3072 bits or Ed25519 is the current advice.
- **Declared-vs-embedded mismatch** — the algorithm written at the start of the line
  disagrees with the algorithm inside the decoded blob.
- **Certificates** — expired, not-yet-valid, or a user certificate with no principals
  (which makes it valid for *any* username).

### Certificates

For a `*-cert-v01@openssh.com` blob you also get the certificate type (`user` or `host`),
serial, key ID, principals, the validity window with a status and days-until-expiry,
critical options, extensions, and the signing CA's algorithm and SHA-256 fingerprint.

Note that the fingerprint reported for a certificate is the fingerprint of the
**certified public key**, not of the certificate envelope — this matches what
`ssh-keygen -l` prints, so the value lines up with the plain `.pub` file the certificate
was issued against.

### Legacy fingerprint options

- **Show the legacy SHA-1 fingerprint** — adds `fingerprint_sha1`, the value
  `ssh-keygen -l -E sha1` prints. Only needed for older tooling.
- **Uppercase the MD5 hex digits** — prints `MD5:AA:BB:…` to match consoles and inventory
  systems that display them that way. The digest itself is unchanged.

### Limits

- Up to **256 KiB** of input and **200 keys** per run.
- Supported algorithms: `ssh-ed25519`, `ssh-rsa`, `ssh-dss`,
  `ecdsa-sha2-nistp256/384/521` and the `sk-*` FIDO variants.
- **Public keys only.** Private keys are rejected on purpose — see the FAQ.
- PEM/PKCS#8 public keys (`-----BEGIN PUBLIC KEY-----`) are not parsed; convert them
  first with `ssh-keygen -i -m PKCS8`.

Also available from the gizza CLI and in chat.

## FAQ

<details>
<summary>Is it safe to paste a key here?</summary>

For a **public** key, yes — parsing happens entirely in your browser via WebAssembly, so
nothing is uploaded, logged or stored on a server. A public key is also, by design, not a
secret: it's the half you hand out.

Private keys are a different matter, and this tool refuses to parse them at all. If you
paste one, you get an error telling you so rather than a decode.

</details>

<details>
<summary>Why does it reject my private key?</summary>

Because a private key should never be pasted into a web page, and a tool that quietly
accepted one would be training the wrong habit. If the input starts with
`-----BEGIN OPENSSH PRIVATE KEY-----` (or any other private-key armor), the parser stops
and tells you to use the matching `.pub` file instead.

If you've lost the `.pub` file, regenerate it locally with
`ssh-keygen -y -f ~/.ssh/id_ed25519`, then paste that.

</details>

<details>
<summary>How is this different from running ssh-keygen -l?</summary>

The fingerprints are identical — that's the point, and the parser follows `ssh-keygen`
semantics exactly, including the certificate rule described above. What you get on top is
everything `ssh-keygen -l` doesn't print: the source format it detected, the
`authorized_keys` options or `known_hosts` host patterns, FIDO application strings for
`sk-*` keys, full certificate details, a strength rating with warnings, and MD5 and SHA-1
alongside SHA-256 in one pass.

It also works on a key you were *sent*, without first saving it to a file — and on a
machine where you don't have `ssh-keygen`.

</details>

<details>
<summary>My key's comment is missing from the output. Where did it go?</summary>

The comment field is optional in every SSH key format, and it's routinely stripped by
tooling that round-trips keys — cloud consoles, config-management templates and copy-paste
through chat clients all drop it. If `comment` is absent from the report, the key you
pasted genuinely doesn't carry one.

The comment is also **not** part of the key, so it never affects the fingerprint. Two
lines with the same base64 blob and different comments are the same key.

</details>

<details>
<summary>Can I paste a whole authorized_keys or known_hosts file?</summary>

Yes — up to 200 keys and 256 KiB. Blank lines and `#` comment lines are skipped, and each
entry is reported separately with a 1-based `index` so you can map results back to
line order. If one entry is malformed, the rest are still parsed and the bad one is
flagged with a warning rather than failing the whole run.

Comparing `key_count` against `unique_fingerprints` is a quick way to spot duplicate
authorisations in a file that's grown over the years.

</details>

<details>
<summary>What does a hashed |1| known_hosts entry give me?</summary>

Everything except the hostname. OpenSSH's `HashKnownHosts` replaces host patterns with an
HMAC-SHA1 of the hostname, which is a one-way function — the parser reports
`hashed_hosts: true` and the key details, but the original host cannot be recovered from
the entry by this tool or any other.

To find the line for a specific host in a hashed file, use `ssh-keygen -F hostname`, which
recomputes the HMAC with each line's salt and compares.

</details>

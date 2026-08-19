## About this tool

Encrypt the **values** of a YAML, JSON or `.env` file with a passphrase while leaving
every **key** in the clear. Instead of turning the whole file into one opaque blob,
each selected leaf value is replaced with a marker:

```text
password: ENC[GZAE1,data:...,iv:...,tag:...,type:str]
```

The result is still valid YAML, JSON or `.env`, so it can be committed, reviewed, and
diffed — a code review shows *which* secret changed without showing the secret. A small
metadata block (`gizza_sops` in YAML/JSON, `GIZZA_SOPS_*` keys in `.env`) records the KDF
salt so the same passphrase can decrypt it later.

Under the hood: one AES-256 key per document derived with PBKDF2-HMAC-SHA256 (200,000
iterations) over a fresh 16-byte random salt, then AES-256-GCM per value with a fresh
96-bit IV. Each value's dotted key path is used as the authenticated data, so a ciphertext
copied from one key onto another fails to decrypt instead of silently succeeding. The
recorded `type:` restores integers, floats and booleans as their original types on decrypt.

Typical run:

1. Paste your config into the document field.
2. Enter a passphrase you can reproduce later, and keep **Mode** on `Encrypt values`.
3. Leave **Document format** on `Auto-detect`, or pin it if the file is ambiguous.
4. Copy the rewritten document. To reverse it, paste it back and switch Mode to
   `Decrypt values` with the same passphrase.

Choosing what gets encrypted — set **one** rule at a time, or none:

- `Encrypt only keys ending with` — e.g. `_secret`: nothing else is touched.
- `Leave keys ending with in the clear` — defaults to `_unencrypted`; clear it to encrypt
  every value.
- `Encrypt only keys matching` — a regular expression, e.g. `^(password|token|.*_key)$`.
- `Leave keys matching in the clear` — a regular expression, e.g. `^(host|region|port)$`.

A rule matches against the key **and its ancestors**, so exempting `public_unencrypted`
exempts the whole subtree beneath it. Setting two rules at once is rejected rather than
silently resolved.

Limits and edge cases: documents are capped at 2 MiB. The YAML and JSON roots must be a
mapping/object so the metadata block has somewhere to live. `null` values are skipped, and
list items are walked by index. Comments and key order survive in `.env`; YAML and JSON
are re-serialized, so comments and exotic formatting in those two are not preserved.
Encrypting an already-encrypted document is refused, and so is decrypting a document with
no metadata block. Every run produces different ciphertext for the same input — that is the
fresh salt and IV doing their job, not a bug.

This is a passphrase-based container format, deliberately **not** the on-disk format of the
`sops` command-line tool, whose metadata describes KMS, `age` or PGP keys. Output from this
page is not interchangeable with that CLI.

## FAQ

<details>
<summary>Can the sops CLI read a file encrypted here?</summary>

No. This page uses its own `ENC[GZAE1,...]` marker and a passphrase-derived key, because a
passphrase has no place in the KMS/`age`/PGP metadata that the `sops` binary expects.
Reusing that marker would claim compatibility that does not exist. Decrypt here, with the
same passphrase.

</details>

<details>
<summary>Why are the keys still readable?</summary>

That is the point of value-level encryption. Readable keys mean the file still parses,
still validates against a schema, and still produces a meaningful `git diff` — you can see
that `database.password` was rotated without anyone seeing the password. If you need the
key names hidden too, encrypt the whole file with a file-level tool instead.

</details>

<details>
<summary>What happens if I lose the passphrase?</summary>

The values are unrecoverable. The key is derived from the passphrase alone; only the salt
is stored in the document, never the key or a verifier. Store the passphrase in a password
manager or a secrets service before you encrypt anything you care about.

</details>

<details>
<summary>Why does encrypting the same file twice give different output?</summary>

Each run draws a new random salt and a new random IV for every value. Identical output
would leak that two values are the same, so the difference is intentional. Both outputs
decrypt to the same document with the same passphrase.

</details>

<details>
<summary>Can I move an encrypted value to a different key?</summary>

No, and that is enforced. Each value is bound to its dotted key path through AES-GCM's
additional authenticated data, so a marker pasted under a different key fails to decrypt
with a clear error. Rename a key by decrypting, renaming, and encrypting again.

</details>

<details>
<summary>Is my passphrase safe to put in a shared link?</summary>

No. The page can pre-fill fields from the URL, which is handy for a local test but means
the passphrase would sit in browser history, bookmarks, and anything the link is pasted
into. Type it into the form instead, and never share a link that carries both the passphrase
and the encrypted document.

</details>

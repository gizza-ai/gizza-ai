## Inspect OpenPGP key metadata locally

Use this PGP key info tool to check an ASCII-armored OpenPGP public or private
key before you trust, import, or share it. The tool reports the primary
fingerprint, short key ID, public-key algorithm, creation and expiry dates, user
IDs, and subkey fingerprints/capabilities.

Everything runs in your browser with WebAssembly. The pasted key is not uploaded
to a server, and no account or network lookup is required.

### FAQ

<details>
<summary>What can I paste?</summary>

Paste a full <code>-----BEGIN PGP PUBLIC KEY BLOCK-----</code> or
<code>-----BEGIN PGP PRIVATE KEY BLOCK-----</code>. For private keys, the tool only
derives and displays public metadata; it does not decrypt or export secret
material.

</details>

<details>
<summary>Why check the fingerprint?</summary>

The full fingerprint is the safest compact identifier for an OpenPGP key. Use
it when comparing a key against an independently published fingerprint or when
checking that a signing/encryption key is the one you expected.

</details>

<details>
<summary>How is the expiry date determined?</summary>

OpenPGP stores expiry as a validity period relative to the key's creation time, so the tool computes <code>created_at + validity days</code> and shows it as an RFC-3339 timestamp. A key with no validity period shows no <code>expires_at</code> at all — it never expires. Primary key and subkeys are reported separately, so a subkey can expire before the primary.

</details>

<details>
<summary>What does "key failed self-verification" mean?</summary>

Every OpenPGP key carries self-signatures binding its user IDs and subkeys to the primary key. The tool verifies those before reporting anything; if verification fails, the key is corrupted, truncated (a partial paste is common), or has been tampered with — don't import it.

</details>

<details>
<summary>What do the subkey capabilities "sign" and "encrypt" mean?</summary>

They are the flags each subkey was created with: a <code>sign</code> subkey can issue signatures, an <code>encrypt</code> subkey is the one others encrypt messages to. A typical modern key has a signing-capable primary key plus a separate encryption subkey.

</details>

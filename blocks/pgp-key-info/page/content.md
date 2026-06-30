## Inspect OpenPGP key metadata locally

Use this PGP key info tool to check an ASCII-armored OpenPGP public or private
key before you trust, import, or share it. The tool reports the primary
fingerprint, short key ID, public-key algorithm, creation and expiry dates, user
IDs, and subkey fingerprints/capabilities.

Everything runs in your browser with WebAssembly. The pasted key is not uploaded
to a server, and no account or network lookup is required.

<details>
<summary>What can I paste?</summary>
<p>Paste a full <code>-----BEGIN PGP PUBLIC KEY BLOCK-----</code> or
<code>-----BEGIN PGP PRIVATE KEY BLOCK-----</code>. For private keys, the tool only
derives and displays public metadata; it does not decrypt or export secret
material.</p>
</details>

<details>
<summary>Why check the fingerprint?</summary>
<p>The full fingerprint is the safest compact identifier for an OpenPGP key. Use
it when comparing a key against an independently published fingerprint or when
checking that a signing/encryption key is the one you expected.</p>
</details>

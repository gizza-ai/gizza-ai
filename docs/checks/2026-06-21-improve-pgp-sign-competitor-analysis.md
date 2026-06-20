# pgp-sign — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/pgp-sign` — create a detached or clear-signed OpenPGP
signature over a message with a private key. Pure-Rust (rPGP). **Chat + CLI only,
no page** (see "Honest scope"). Sibling of `pgp-encrypt`.

## What competitors do

- **GnuPG (`gpg --sign` / `--clearsign` / `--detach-sign --armor`)** — the
  reference tool, local and correct, but needs GnuPG installed, the private key
  imported into a keyring, and CLI know-how.
- **Online "PGP sign" tools** — paste a private key + message, get a signature.
  **Major weakness: you paste your *private key* into a third-party web page** —
  the single worst thing you can do with a signing key.
- **Sequoia (`sq sign`), OpenPGP.js web apps** — capable, but native installs /
  keyrings or full JS apps.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (rPGP) compiled to wasm: runs
   in the chat Service Worker and headless in the CLI. The private key and message
   never leave the device — critical, since this tool by definition handles a
   *private* key.
2. **No keyring required.** Paste the ASCII-armored private key directly; no
   `gpg --import`, no keyring state to manage.
3. **Both signature shapes in one tool.** `detached` produces a standalone
   `-----BEGIN PGP SIGNATURE-----` that verifies the original, unmodified bytes
   (the form used to sign releases, packages, commits); `clearsign` produces an
   inline `-----BEGIN PGP SIGNED MESSAGE-----` that keeps the text human-readable
   with the signature appended (the form used for signed emails / announcements).
4. **Passphrase-aware.** Unlocks a protected private key with a supplied
   passphrase; works directly with unprotected keys too.
5. **Interoperable output.** Standard ASCII armor, the key's preferred hash —
   verifies with GnuPG (`gpg --verify`), Sequoia, etc.

## Honest scope

- **No page (chat + CLI only).** An armored private key is a multi-line block; the
  tool-page framework's text field is single-line and strips newlines on paste,
  which corrupts the armor. Rather than ship a page that can't accept a real key,
  this tool is chat + CLI only — matching `pgp-encrypt` / `generate-rsa-key-pair`.
- **Signs with the primary key.** Uses the key's primary key (the usual signing
  key for GPG/Proton keys, which are sign+certify); a dedicated signing *subkey*
  is not separately selected.
- **No verification / no encryption** — signing only (see `pgp-encrypt` for the
  encryption side).

## Tests

4 core unit tests using a **key pair generated in-test with rPGP**: a detached
signature **verifies** against the original bytes with the public key *and* fails
against tampered data; a clearsigned message round-trips (verifies) and keeps the
readable text; clear errors on empty / non-key / public-key-instead-of-private
input; and `Mode::parse` accepts the documented aliases. Plus the block
drift-guard schema test. **CLI verified** end-to-end for both modes against a
throwaway armored private key (generated via the `core` `gensec` example):
detached → `-----BEGIN PGP SIGNATURE-----`, clearsign → `-----BEGIN PGP SIGNED
MESSAGE-----`. `wafer build` instantiates the chat block in the wafer runtime
(2.22 MiB).

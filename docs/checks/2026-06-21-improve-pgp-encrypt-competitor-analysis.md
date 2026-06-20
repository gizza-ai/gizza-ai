# pgp-encrypt — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/pgp-encrypt` — encrypt a message to one or more OpenPGP
public keys, producing an ASCII-armored `-----BEGIN PGP MESSAGE-----` block.
Pure-Rust (rPGP). **Chat + CLI only, no page** (see "Honest scope").

## What competitors do

- **Online "PGP encrypt" tools** (e.g. various web encrypters) — paste a public
  key + message, get armored ciphertext. Strength: zero install. **Weakness: the
  plaintext (the thing you're trying to protect) is typed into a web page you
  don't control** — the whole point of PGP is undermined if the page is
  untrustworthy or ad-supported.
- **GnuPG (`gpg --encrypt --armor -r ...`)** — the reference implementation,
  local and correct, but requires installing GnuPG, importing keys into a
  keyring, and knowing the CLI. No "just paste an armored key" path.
- **Sequoia (`sq encrypt`), `gpg`-wrappers, OpenPGP.js web apps** — capable, but
  either need native installs/keyrings or are full JS web apps.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (rPGP) compiled to wasm: runs
   in the chat Service Worker and headless in the CLI. The plaintext and the keys
   never leave the device — the property PGP is supposed to give you, preserved.
2. **No keyring required.** Paste one or more ASCII-armored public keys directly;
   the tool parses them on the spot. No `gpg --import`, no keyring state.
3. **Multi-recipient in one step.** Concatenate several armored public keys and
   the message is encrypted so that *each* recipient can decrypt it with their own
   private key (one PKESK per recipient). For each key the tool selects the right
   encryption (sub)key automatically — the encryption subkey if present (the
   standard GPG/Sequoia/Proton layout), else an encryption-capable primary.
4. **Interoperable output.** AES-256, SEIP v1 packets, standard ASCII armor — the
   result decrypts with GnuPG (`gpg --decrypt`), Sequoia, ProtonMail, etc.
5. **Agent- + automation-friendly.** Message + keys in, armored block out — usable
   by an LLM or a CI step, identical via chat and CLI.

## Honest scope

- **No page (chat + CLI only).** A real OpenPGP public key is a multi-line
  armored block; the tool-page framework's text field is single-line and strips
  newlines on paste, which corrupts the armor. Rather than ship a page that can't
  accept a real key, this tool is chat + CLI only — matching the other key-centric
  crypto tools (`generate-rsa-key-pair`, `encrypt-file`).
- **Encrypt only, no signing.** The message is encrypted but not signed; the tool
  does not prove the sender's identity. (Decryption is done with the user's own
  PGP client / private key.)
- **No compression / no password (symmetric) mode**; public-key encryption only.

## Tests

3 core unit tests using **key pairs generated in-test with rPGP** (RSA primary +
RSA encryption subkey): single-recipient round-trip (encrypt → decrypt with the
matching secret key → plaintext matches), two-recipient round-trip (both secret
keys independently decrypt the same message), and clear errors on empty / non-key
input. Plus the block drift-guard schema test. **CLI verified** end-to-end: a
throwaway armored public key (generated via the `core` `genkey` example) →
`gizza tool pgp-encrypt` produces a valid `-----BEGIN PGP MESSAGE-----` block.
`wafer build` instantiates the chat block in the wafer runtime (1.93 MiB) —
confirming rPGP runs under wasm32-wasip1, not just compiles.

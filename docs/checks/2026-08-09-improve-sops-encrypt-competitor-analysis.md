# sops-encrypt — competitor analysis (2026-08-09)

Scan run BEFORE implementation, per `.claude/skills/create-next-tool/SKILL.md` step 3 /
`/improve-tool` Phases 2–3. Everything below is **paraphrased**; no competitor copy, branding
or trademark text is reproduced or shipped in the tool.

Backlog row: `sops-encrypt` — "Encrypts only the values in a YAML, JSON, or .env file with a
passphrase/key, leaving keys readable." (type_hint `pure`).

## Duplicate check (done first)

`ls blocks/ | grep -iE 'sops|encrypt|crypt|secret|vault|seal'` → `age-encrypt`, `encrypt-file`,
`text-encrypt`, `nacl-secretbox-encrypt`, `aes-cipher`, `encrypted-zip`, `pgp-encrypt`,
`rsa-encrypt`, `secret-scanner`, plus `dotenv-manager`, `env-file-merger`, `json-redact`,
`json-mask`.

All of the encryptors are **whole-blob**: they take bytes/text in and return one opaque
ciphertext. None preserves document structure. `json-redact`/`dotenv-manager` mask secrets but
**destructively** — the value cannot be recovered. Nothing in `blocks/` does
structure-preserving, per-value, reversible encryption, which is this tool's entire point
(readable keys → readable git diffs). **Not a duplicate — built.**

## Competitors surveyed (3)

### 1. SOPS (getsops, CNCF sandbox) — the reference implementation
- Encrypts **only leaf values** of YAML / JSON / ENV / INI trees; keys stay cleartext so diffs
  and code review stay meaningful. Whole-file (`BINARY`) mode also exists.
- On-disk value shape is a self-describing marker containing the algorithm, the ciphertext, the
  per-value IV, the auth tag, and the **original scalar type** — so `decrypt` restores an int as
  an int, a bool as a bool.
- A trailing metadata section records the key-management entries (KMS / age / PGP), a file MAC,
  version and timestamp.
- Selective-encryption flags: `--encrypted-suffix`, `--unencrypted-suffix` (documented default
  behaviour: keys ending `_unencrypted` are left in the clear), `--encrypted-regex`,
  `--unencrypted-regex`, plus YAML-only `--encrypted-comment-regex` / `--unencrypted-comment-regex`,
  and `--mac-only-encrypted`.
- Verbs: `encrypt` (with in-place `-i`), `decrypt`, `edit` (decrypt → $EDITOR → re-encrypt),
  `rotate`.
- Key management is **external**: AWS/GCP/Azure/Huawei KMS, age, PGP. There is no passphrase mode.

### 2. dotenvx — `.env`-focused
- `dotenvx encrypt` rewrites a `.env` in place: keys stay readable, values become a prefixed
  ciphertext token, and a public key line is added at the top of the file.
- Asymmetric: the public key (safe to commit, lets anyone add a secret) lives in the file; the
  private key lives in a separate uncommitted key file.
- Scope-limited to dotenv; no YAML/JSON tree walking, no per-key selection flags.

### 3. Ansible Vault (`ansible-vault encrypt_string`)
- Encrypts a **single value** and prints an inline YAML snippet (a tagged block scalar) to paste
  into an otherwise-plaintext vars file — the closest thing to a passphrase-based per-value
  encryptor.
- Passphrase-driven (interactive prompt, password file, or script), with named vault IDs.
- Manual: it does not walk a document and encrypt every leaf; you run it per value and paste.
  Decryption happens at playbook runtime, not via a general "decrypt this file" verb.

## Table stakes → in-model / out-of-model

| Capability (paraphrased) | Seen in | Decision |
|---|---|---|
| Encrypt only leaf values, keys stay cleartext | SOPS, dotenvx | **In** — core behaviour |
| YAML + JSON + `.env` inputs | SOPS | **In** — all three, plus `format=auto` detection |
| Reversible decrypt of the same document | SOPS, dotenvx | **In** — `mode=decrypt` |
| Self-describing per-value marker with IV + auth tag | SOPS | **In** — `ENC[GZAE1,data:…,iv:…,tag:…,type:…]` |
| Original scalar type restored on decrypt (int/float/bool/str) | SOPS | **In** — `type:` field |
| Per-value authentication bound to its key path | SOPS (AAD) | **In** — the dotted key path is the GCM AAD, so a value cannot be swapped between keys |
| `unencrypted_suffix` (default: skip keys ending `_unencrypted`) | SOPS | **In** — same default |
| `encrypted_suffix` (encrypt only matching keys) | SOPS | **In** |
| `encrypted_regex` / `unencrypted_regex` | SOPS | **In** — both, via the `regex` crate |
| Error when several selection rules are combined | SOPS | **In** — one rule at a time |
| Passphrase-based key derivation | Ansible Vault | **In** — PBKDF2-HMAC-SHA256, 200k iterations, one random file salt, one derived data key per document |
| Nested maps + arrays walked to their leaves | SOPS | **In** — array elements get numeric path segments |
| Comment / blank-line preservation in `.env` | SOPS, dotenvx | **In** — the `.env` path is a line-preserving rewriter (`export ` prefix and quoting preserved) |
| KMS / age / PGP key management, key rotation | SOPS | **Out of model** — needs cloud credentials and network; gizza blocks are offline pure-compute. Passphrase only. |
| Binary/whole-file mode | SOPS | **Out of model here** — already covered by `blocks/encrypt-file` and `blocks/text-encrypt`; pointing users there beats a fourth whole-blob encryptor |
| In-place edit / `$EDITOR` round trip, in-place file writes | SOPS, dotenvx | **Out of model** — no filesystem or interactive editor on any gizza surface (page/CLI/chat are single-shot text in → text out) |
| Comment-driven selection (`--encrypted-comment-regex`) | SOPS | **Out of model** — the YAML path round-trips through a typed tree (`serde_yml`), which does not retain comments; see the stated limits on the page |
| YAML comment/anchor/formatting preservation | SOPS | **Out of model** for YAML (stated as a limit on the page); `.env` comments *are* preserved because that path is line-based |
| File-level MAC over the whole document | SOPS | **Out of model** — each value carries its own GCM tag, and the key-path AAD covers reordering/renaming; a separate whole-file MAC would add a second failure mode without new protection at this scope |
| Interop with the `sops` binary itself | — | **Out of model, stated plainly** — real SOPS requires a KMS/age/PGP-wrapped data key; a passphrase-derived key has no place in its metadata block. Our marker is deliberately `GZAE1`, not `AES256_GCM`, so an encrypted file is never mistaken for one `sops` can open. |

## UX patterns adopted

- **Preset chips** (`[[example]]`) instead of copy: the competitors ship documented recipes rather
  than GUI presets, so the page seeds one chip per real workflow — encrypt a YAML config, encrypt
  a `.env`, encrypt only `*_key`-suffixed values, and decrypt a sample document.
- **`multiline = true`** on the document field (pasted YAML/JSON/.env keeps its newlines) and on
  nothing else.
- **Enum `<select>`s** for `mode` and `format` with friendly `[input.labels]`, matching how the
  competitors' flags are really a small closed set.
- **Placeholders** on every text field, including a realistic suffix/regex hint.
- Deliberately **not** in the page URL: the passphrase. The deep-link Playwright case uses
  `?format=` / `?mode=`, and the page copy says not to put a passphrase in a shared link.

## In-model decisions recorded

1. Own, documented container format (`ENC[GZAE1,…]` + a `gizza_sops` metadata block, or
   `GIZZA_SOPS_*` keys for `.env`) — familiar in shape, unmistakably not `sops`-compatible.
2. One PBKDF2 derivation per document (not per value) — 200k iterations stays fast for a
   200-key file while each value still gets a fresh random 96-bit IV.
3. Nulls are left untouched (nothing to hide in a null, and it keeps the shape obvious); stated
   on the page.
4. Idempotence guards: encrypting an already-`ENC[…]` document is refused with an actionable
   error rather than double-encrypting; decrypting a document with no metadata block likewise.

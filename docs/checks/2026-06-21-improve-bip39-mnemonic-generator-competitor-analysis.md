# bip39-mnemonic-generator — competitor analysis (2026-06-21)

Tool: generate a BIP39 mnemonic seed phrase (12–24 words) from secure entropy, with the
BIP39 checksum word and the derived 512-bit seed. Pure Rust (getrandom CSPRNG + sha2 +
pbkdf2-hmac-sha512), runs on all backends. Surfaces: chat + CLI + page.

## Surface verification (Phase 1)

- **Chat block** — `wafer build` validates `target/block.wasm` instantiates (344 KiB); drift
  test `schema_json_matches_authored_chat_schema` passes (no LLM-facing schema drift).
- **CLI** — `gizza tool bip39-mnemonic-generator entropy_hex=000…0 passphrase=TREZOR` returns the
  canonical BIP39 vector mnemonic `abandon … about` and seed `c55257c3…7463b04`; `strength=256`
  → 24 words / 32 entropy bytes; `strength=160` → 15 words. All exact.
- **Page** — Playwright `tool-page-bip39-mnemonic-generator.spec.ts` passes: deterministic vector
  path (all-zero entropy + TREZOR) and the random 256-bit → 24-word path.
- **Correctness** — unit tests assert two official Trezor BIP39 test vectors (all-zero 128-bit and
  all-0xff 256-bit, mnemonic + seed), wordlist = 2048 words, checksum, and passphrase-changes-seed.
  Embedded English wordlist matches the canonical BIP39 sha256.

## Top competitors surveyed

1. **iancoleman.io/bip39** — the reference implementation. Strengths: all 5 word counts, passphrase,
   live entropy input (dice/hex/binary), AND full downstream **BIP32/BIP44 HD derivation** (xprv/xpub,
   derivation paths, per-coin addresses), multi-language wordlists, mnemonic→entropy reverse.
2. **getcoinplate.com BIP39 generator** — offline/online, word-count picker, passphrase, print-friendly,
   strong "generate offline" security messaging.
3. **it-tools.tech / cyberchef.dev / devtoolcafe BIP39 generator** — minimal: word count + passphrase →
   mnemonic + seed hex. Closest in scope to this tool.
4. **8gwifi.org BIP39 Mnemonic Generator & Validator** — adds **validation** (checksum check of a pasted
   phrase) alongside generation.
5. **bip39-phrase.com** — generator plus the full 2048-word reference list as content.

## Gap analysis (fit-to-model)

Closed / already in-model (shipped):
- All 5 entropy strengths (128/160/192/224/256 → 12/15/18/21/24 words) via the `strength` enum.
- Optional **passphrase** (BIP39 "25th word") mixed into the PBKDF2-HMAC-SHA512 seed.
- **512-bit seed (hex)** output, not just the words (matches it-tools/cyberchef/devtoolcafe).
- **Deterministic entropy input** (`entropy_hex`) — recovery / test-vector reproduction; equivalent to
  iancoleman's "enter your own entropy in hex". Page-testable because of it.
- Returns entropy hex + word count + strength alongside the mnemonic (structured chat/CLI output).

Out-of-model / deliberately NOT built (would need new crates/surfaces, scope creep):
- **BIP32/BIP44 HD derivation** (xprv/xpub, derivation paths, coin addresses) — needs secp256k1 HD key
  derivation + per-coin address encoding; a separate large tool, not a mnemonic generator. Out of scope.
- **Multi-language wordlists** (Japanese/Korean/Chinese/…) — would 9× the embedded data and the seed
  uses NFKD normalization per language; English only for now (the dominant default).
- **Mnemonic validation / reverse (phrase → entropy)** — a distinct "validate/recover" tool; could be a
  future sibling tool rather than overloading the generator.
- **Dice / coin / binary entropy input UI** — the `entropy_hex` field already covers "bring your own
  entropy"; a dice widget is page-only UX sugar.

## Conclusion

The tool matches or exceeds the minimal-generator competitors (it-tools, cyberchef, devtoolcafe) on the
in-model surface — all word counts, passphrase, seed derivation, and deterministic custom entropy — with
official-test-vector-verified correctness across chat, CLI, and page. The remaining competitor features
(HD derivation, multi-language, validation) are separate tools, not in-model gaps for a generator.

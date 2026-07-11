# bip39-seed-derive — competitor analysis (2026-07-11)

Function: validate an existing BIP39 mnemonic phrase and derive the 512-bit BIP39 seed from the mnemonic plus an optional passphrase using PBKDF2-HMAC-SHA512 (2048 iterations). Pure Rust, browser-local.

## Competitors scanned (top 3)

1. **Ian Coleman's BIP39 Mnemonic Code Converter** — derives seed, BIP32 roots, addresses, and wallet paths from a mnemonic/passphrase.
2. **Trezor python-mnemonic test vectors/library** — reference BIP39 validation and mnemonic-to-seed derivation behavior.
3. **Mnemonic Code Converter / BIP39 online calculators** — common web tools that accept mnemonic + optional passphrase and display seed/entropy.

## Table-stakes feature matrix

| Capability | Coleman tool | Trezor reference | Generic calculators | in gizza | fit |
|---|---|---|---|---|---|
| Accept pasted 12/15/18/21/24-word mnemonic | ✅ | ✅ | ✅ | ✅ | in-model |
| Optional BIP39 passphrase / “25th word” | ✅ | ✅ | ✅ | ✅ | in-model |
| PBKDF2-HMAC-SHA512, 2048 rounds, 64-byte seed | ✅ | ✅ | ✅ | ✅ | in-model |
| Official English wordlist membership validation | ✅ | ✅ | ✅ | ✅ | in-model |
| Checksum validation and clear typo errors | ✅ | ✅ | ✅ | ✅ | in-model |
| Normalize whitespace/case before derivation | ✅ | ✅ | ✅ | ✅ | in-model |
| Show seed as 128 hex chars | ✅ | ✅ | ✅ | ✅ | in-model |
| Show recovered entropy hex / strength | ✅ | ✅ | ➖ | ✅ | in-model |
| Worked test vector (abandon…about + TREZOR) | ✅ | ✅ | ✅ | ✅ | in-model |
| Derive BIP32 xprv / addresses / paths | ✅ | ➖ | mixed | ❌ | out-of-scope |
| Generate a new mnemonic | ✅ | ✅ | mixed | ❌ (separate tool) | existing sibling |

## Decisions

- **Scope**: this tool derives the BIP39 seed from an existing phrase. New phrase generation remains in `bip39-mnemonic-generator`; wallet path/address derivation is intentionally out-of-scope and belongs to separate BIP32/BIP44 tooling.
- **Validation-first UX**: unknown word, invalid word count, and checksum failures are hard errors before any seed is emitted.
- **Outputs**: normalized mnemonic, word count, recovered entropy hex, passphrase marker, and seed hex. The seed is the main output; entropy/word count help users verify they pasted the intended phrase.
- **Privacy**: page copy and docs emphasize local WebAssembly execution and caution that real wallet phrases are secrets.

## UX

- `mnemonic` is a multiline textarea so 12–24 words can be pasted with line wraps.
- `passphrase` is a plain optional text field.
- Example chips include the canonical Trezor `abandon … about` vector and a 24-word vector.

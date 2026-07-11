## About this tool

This tool takes a **BIP39 mnemonic recovery phrase you already have** and derives
the **512-bit BIP39 seed** — the master secret that Bitcoin and virtually every
modern hierarchical-deterministic (HD) wallet expand into every private key
(BIP32 / BIP44). Before deriving, it checks that the phrase is a valid BIP39
mnemonic: the right word count, every word in the official 2048-word English
list, and a passing checksum. Need to *create* a brand-new phrase instead? Use the
[BIP39 mnemonic generator](/tools/bip39-mnemonic-generator/).

## How it works

1. **Validate** — the phrase is normalized (lowercased, whitespace collapsed) and
   checked: it must be 12, 15, 18, 21, or 24 words, every word must exist in the
   BIP39 English wordlist, and the trailing checksum bits — the first *ENT/32* bits
   of `SHA-256(entropy)` — must match. Any failure is reported instead of a seed.
2. **Recover entropy** — each word maps to an 11-bit index; the leading bits
   reconstruct the original 128–256 bits of entropy shown in the output.
3. **Derive the seed** — the mnemonic plus your optional passphrase is stretched
   with `PBKDF2-HMAC-SHA512` (2048 iterations, salt `"mnemonic"` + passphrase)
   into the 64-byte (512-bit) seed.

## Options

- **Mnemonic phrase** — paste your 12–24 word BIP39 phrase. Case and stray spaces
  are tolerated; unknown words and bad checksums are rejected with a clear message.
- **Passphrase** — the optional BIP39 "25th word." It never changes the words, but
  it is mixed into the derivation, so the same phrase with a different passphrase
  produces a completely different seed (and therefore a different wallet).

## Privacy

Everything runs locally in your browser via WebAssembly. Your mnemonic, passphrase,
and the derived seed never leave your device. **Never** paste a phrase that guards
real funds into any website you do not fully trust — prefer an offline machine or a
hardware wallet for anything of value.

## FAQ

<details>
<summary>What is the difference between the mnemonic, the entropy, and the seed?</summary>

The **entropy** is the original 128–256 random bits. The **mnemonic** is those
bits (plus a checksum) encoded as 12–24 memorable words. The **seed** is a separate
512-bit value produced by running the mnemonic and an optional passphrase through
`PBKDF2-HMAC-SHA512`. Wallets store or back up the mnemonic, but they derive all
keys from the seed. This tool shows all three so you can verify a recovery.

</details>

<details>
<summary>Why was my phrase rejected as invalid?</summary>

Three things must hold for a BIP39 phrase: it has exactly 12, 15, 18, 21, or 24
words; every word appears in the BIP39 English wordlist; and the built-in checksum
passes. A rejection usually means a word is misspelled, a word is in the wrong
position, or a word is missing or duplicated. The error names the failing word or
the specific rule so you can fix it. Only the English wordlist is supported.

</details>

<details>
<summary>Does the passphrase change the words or just the seed?</summary>

Only the seed. The passphrase (the "25th word") is never part of the mnemonic and
never alters the words — it is folded into the PBKDF2 stretch, so the same 12–24
words with a different passphrase yield a completely different 512-bit seed and
therefore a different wallet. If you lose the passphrase, the words alone cannot
restore that wallet, so record it as carefully as the phrase itself.

</details>

<details>
<summary>Can I check this against a known BIP39 test vector?</summary>

Yes. The canonical Trezor vector is the phrase `abandon abandon abandon abandon
abandon abandon abandon abandon abandon abandon abandon about` with passphrase
`TREZOR`, which derives the seed
`c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04`.
The **BIP39 test vector (TREZOR)** example chip loads exactly this so you can
confirm the output matches.

</details>

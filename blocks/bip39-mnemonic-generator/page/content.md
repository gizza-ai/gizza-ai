## About this tool

This generator produces a **BIP39 mnemonic seed phrase** — the human-readable list
of words used by Bitcoin and almost every modern hierarchical-deterministic (HD)
crypto wallet to back up and restore a wallet. Pick a strength and it draws fresh
cryptographically secure entropy, appends the BIP39 checksum, maps the bits to the
official 2048-word English wordlist, and derives the 512-bit BIP39 seed.

## How it works

1. **Entropy** — 128, 160, 192, 224, or 256 bits of random data (more bits → more
   words: 12, 15, 18, 21, or 24).
2. **Checksum** — the first *ENT/32* bits of `SHA-256(entropy)` are appended, so the
   final word carries a checksum that detects typos.
3. **Words** — the combined bits are split into 11-bit groups, each indexing the
   2048-word English list.
4. **Seed** — the mnemonic plus an optional passphrase is stretched with
   `PBKDF2-HMAC-SHA512` (2048 iterations) into the 512-bit seed your wallet uses to
   derive every key (BIP32 / BIP44).

## Options

- **Strength** — choose 12 to 24 words. 12 words (128 bits) is already strong; 24
  words (256 bits) is the common hardware-wallet default.
- **Entropy hex** — leave blank for secure random entropy, or paste your own hex
  (16–32 bytes) to derive a phrase deterministically — useful for recovery checks or
  BIP39 test vectors.
- **Passphrase** — an optional extra secret (the "25th word"). It does not change the
  words but produces an entirely different seed, so the same phrase can guard several
  hidden wallets.

## Privacy

Everything runs locally in your browser via WebAssembly. No seed phrase, passphrase,
or entropy ever leaves your device. **Never** type a real seed phrase into any website
you do not fully trust, and store backups offline.

## FAQ

<details>
<summary>Should I pick 12 or 24 words?</summary>

The strength setting maps entropy bits to words: 128 → 12, 160 → 15, 192 → 18,
224 → 21, 256 → 24. The default of 128 bits (12 words) is already computationally
infeasible to brute-force; 24 words (256 bits) is simply the convention most hardware
wallets ship with. Note that strength is ignored whenever you supply your own entropy
hex — the word count then follows the entropy length.

</details>

<details>
<summary>Can I regenerate the exact same mnemonic later?</summary>

Yes — paste the entropy shown with a generated phrase (or your own) into the
**Entropy hex** field. It must be exactly 16, 20, 24, 28, or 32 bytes of hex
(128–256 bits); the tool then derives the mnemonic deterministically, which is how
you verify recovery or reproduce BIP39 test vectors. Leaving the field blank always
draws fresh secure random entropy.

</details>

<details>
<summary>Does the passphrase change my seed words?</summary>

No. The passphrase (the "25th word") never alters the mnemonic itself — it is mixed
into the PBKDF2-HMAC-SHA512 stretch, so the same 12–24 words plus a different
passphrase yield a completely different 512-bit seed and therefore a different
wallet. Lose the passphrase and the words alone cannot restore that wallet.

</details>

<details>
<summary>Is it safe to generate a real wallet phrase in a browser?</summary>

The generation itself is safe in the sense that it runs entirely in local WebAssembly
using the OS's cryptographically secure RNG, and nothing is transmitted. For
meaningful funds, though, best practice is still to generate on a hardware wallet or
an offline machine — a browser environment has a larger attack surface (extensions,
malware) than a dedicated signer.

</details>

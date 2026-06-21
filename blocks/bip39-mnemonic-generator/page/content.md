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

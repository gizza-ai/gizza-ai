# BIP32 HD key derivation

Derive a Bitcoin BIP32 child private key from either a hex seed or an extended private key (`xprv` / `tprv`). The tool returns the child `xprv`, `xpub`, raw private key, compressed public key, WIF, fingerprint, and one Bitcoin address for the selected address format.

Everything runs locally in the browser. Private keys and seeds are not uploaded.

## Worked example

Use the BIP32 test-vector seed:

- `seed`: `000102030405060708090a0b0c0d0e0f`
- `path`: `m/0'`
- `network`: `mainnet`
- `address_type`: `p2pkh`

The output includes the BIP32 vector child key beginning with `xprv9uHRZZhk6KA...` and the matching `xpub68Gmy5Edvgi...`.

## Inputs

- Provide exactly one of `seed` or `xprv`.
- `path` must start with `m` and may use hardened suffixes `'`, `h`, or `H`.
- `network` changes xprv/xpub version bytes and address prefixes.
- `address_type` renders one address from the derived compressed public key: legacy P2PKH, wrapped SegWit, or native SegWit.

## Limits and edge cases

This is a Bitcoin-only BIP32 private-derivation tool. It does not accept mnemonics directly; use the BIP39 seed tool first, then paste the seed here. It does not derive from xpub-only watch-only keys, address ranges, ypub/zpub version bytes, or altcoin address formats.

<details>
<summary>Can I paste a mnemonic phrase?</summary>

No. First run the mnemonic through the BIP39 seed derivation tool, then paste the resulting 512-bit seed hex here. Keeping mnemonic validation separate keeps the wordlist/checksum logic in one place.
</details>

<details>
<summary>What is the difference between seed and xprv?</summary>

A seed is the root entropy used to create the master BIP32 key. An `xprv` or `tprv` is already an extended private key at some node. This tool can start from either one, but not both at the same time.
</details>

<details>
<summary>Which address type should I choose?</summary>

Use `p2pkh` for legacy `1...` or `m/n...` addresses, `p2sh_p2wpkh` for wrapped SegWit `3...` or `2...` addresses, and `p2wpkh` for native SegWit `bc1...` or `tb1...` addresses.
</details>

<details>
<summary>Can it derive many addresses at once?</summary>

No. Enter one explicit path, such as `m/84'/0'/0'/0/0`. To derive a range, rerun with the last index changed (`.../0`, `.../1`, and so on).
</details>

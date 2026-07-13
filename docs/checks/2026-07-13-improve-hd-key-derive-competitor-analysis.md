# hd-key-derive — competitor analysis (2026-07-13)

Tool: BIP32 hierarchical-deterministic key derivation. Input a hex **seed** (as
produced by `bip39-seed-derive`) or an extended private key (**xprv**), plus a
BIP32 derivation path like `m/44'/0'/0'/0/0`; output the extended keys and the
concrete key/address material at that path. Pure Rust, runs fully offline in the
browser / chat / CLI — no key ever leaves the device.

## Competitor scan (one WebSearch, top real tools)

1. **iancoleman BIP39 / bip32.org** (online web tools). Input a BIP39
   mnemonic **or** a root key (xprv/xpub) **or** a seed, choose a derivation
   path (BIP32 raw, or BIP44/49/84 helpers), and see the account xprv/xpub plus
   a table of derived addresses with their path, address, public key and private
   key (WIF). Client-side only.
2. **dan-da/hd-wallet-derive** (CLI, PHP). Derives BIP32 addresses and private
   keys from an xprv, xpub, or BIP39 mnemonic (+ optional password). Report
   columns: `path`, `address`, `xprv`, `xpub`, `privkey` (WIF), `pubkey`.
   Supports SegWit: `ypub`/p2sh addresses and `zpub`/bech32 addresses, and many
   altcoins via coin-specific version bytes.
3. **Tevm / Voltaire `hdwallet`** (library + docs). BIP32/BIP44 HD wallet:
   derive unlimited child keys from a master seed via secp256k1 EC math; expose
   the extended private/public key, the raw private/public key, and the address
   for a given path.

(All three reachable; none copied — behaviour paraphrased only.)

## Table-stakes → in-model / out-of-model

| Capability | Decision | Where it lands |
|---|---|---|
| Input a hex **seed** | in-model | `seed` param |
| Input a root/extended **xprv** | in-model | `xprv` param |
| BIP32 **derivation path** (`m/44'/0'/0'/0/0`, `'`/`h` hardened) | in-model | `path` param |
| Output extended private key (**xprv**) at the path | in-model | output |
| Output extended public key (**xpub**) at the path | in-model | output |
| Output raw **private key** (hex) and **WIF** (compressed) | in-model | output |
| Output compressed **public key** (hex) | in-model | output |
| Output **address** | in-model | output |
| Address types: legacy **P2PKH**, wrapped **P2SH-P2WPKH**, native **P2WPKH** (bech32) | in-model | `address_type` enum |
| **Network**: mainnet / testnet (version bytes, address prefixes, bech32 HRP) | in-model | `network` enum |
| Key **fingerprint** + depth | in-model | output |

### Out-of-model (listed, not built)

- **Direct BIP39 mnemonic input.** Kept as a separate step: run
  `bip39-seed-derive` (mnemonic → 512-bit seed) and paste the seed here. Keeps
  each tool single-purpose and the wordlist in one place.
- **xpub-only public derivation.** Deriving addresses from a watch-only xpub
  (no private key) needs EC point addition for the non-hardened path; deferred.
  This tool derives from a seed/xprv (private material), which covers the common
  case and can reach any path including hardened levels.
- **Address ranges / gap-limit scans** (derive indices 0..N in one call). This
  tool resolves ONE explicit path; loop the index client-side for a range.
- **ypub/zpub (SLIP-0132) version bytes** and **altcoin** address formats
  (Ethereum, Litecoin, …). Bitcoin mainnet/testnet only; other coins need
  per-coin version-byte/address tables.

## UX controls (matched from competitors)

- `path` is a plain text field with a real placeholder (`m/44'/0'/0'/0/0`).
- `network` and `address_type` render as `<select>` (enum) with friendly labels.
- `[[example]]` preset chips: the BIP32 test-vector seed at `m/0'`, a BIP44
  receive address `m/44'/0'/0'/0/0`, and a native-segwit `m/84'/0'/0'/0/0`.

## Test vectors used

BIP32 test vector 1 (seed `000102...0f`): chain `m` and `m/0'` extended keys
match the spec; the P2PKH address for `m/0'/1/2'/2/1000000000` is checked in
unit + page tests.

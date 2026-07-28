# Competitor analysis — secp256k1-pubkey-derive (2026-07-28)

Tool function: given a secp256k1 **private key**, derive its **public key** point in
compressed (33-byte, `02`/`03`) and uncompressed (65-byte, `04`) SEC1 forms, plus the
raw X / Y coordinates and Y parity. Chain-agnostic (Bitcoin, Ethereum, Tron, …) — the
public-key *point* is identical across chains; only the downstream address encoding differs,
which is out of scope for this tool (covered by sibling blocks).

## Top competitors surveyed

### 1. SecretScan.org — Public Key converter (secretscan.org/PublicKey)
- **Inputs:** hex private key, WIF, and (elsewhere on the site) mnemonic phrases.
- **Outputs:** compressed public key (33 B, `02`/`03`), uncompressed (65 B, `04`), X and Y
  coordinates in hex, and blockchain addresses (Legacy / SegWit Base58 / Bech32 / Taproot)
  across Bitcoin, Ethereum, Tron, BNB, XRP.
- **UX:** single key field → "Result" section with the derived representations; positioned as
  an educational platform. No stated character limits or worked examples on the page.

### 2. learnmeabitcoin.com — Public Key (technical/keys/public-key)
- **Input:** a private key (256-bit number).
- **Outputs:** X and Y coordinates as **decimal and hex**, uncompressed (65 B, `04`),
  compressed (33 B, `02`/`03`), and **X-only** (32 B, for Taproot).
- **Explains:** public key = EC multiply of the secp256k1 generator point G by the private
  scalar; compressed stores only X + a parity byte (`02` = Y even, `03` = Y odd), recoverable
  because the curve's symmetry allows only two Y values per X.
- Worked example values shown for both forms (same X, differing prefix).

### 3. gobittest.appspot.com — Address/key steps
- Step-by-step private-key → public-key derivation (raw point) as part of a full
  address-generation walkthrough. Educational, single-field, shows intermediate hex.

## Table-stakes → decision matrix

| Capability | Competitors | Our decision |
| --- | --- | --- |
| Hex private key input (0x optional) | all | **in-model — built** (`key`, tolerant of `0x`/spaces/`_`) |
| WIF private key input | SecretScan | **in-model — built** (auto-detected base58check WIF) |
| Compressed public key (33 B, `02`/`03`) | all | **in-model — built** |
| Uncompressed public key (65 B, `04`) | all | **in-model — built** |
| X coordinate (hex) | SecretScan, learnmeabitcoin | **in-model — built** |
| Y coordinate (hex) | SecretScan, learnmeabitcoin | **in-model — built** |
| Y parity / prefix explanation (`02`/`03`) | learnmeabitcoin | **in-model — built** (`y_parity: even/odd`) |
| X-only (32 B, Taproot) | learnmeabitcoin | **in-model — built** (the `x` output IS the 32 B X-only key) |
| Select a single representation to copy | SecretScan | **in-model — built** (`format` enum: all/compressed/uncompressed/x/y) |
| Decimal (as well as hex) X/Y | learnmeabitcoin | **considered, rejected** — hex is the on-chain canonical form; decimal is teaching-only and would bloat output. Hex is what every wallet/library consumes. |
| Bitcoin/Ethereum/Taproot **addresses** | SecretScan | **out of scope** — already covered by `bitcoin-address`, `hd-key-derive`, `crypto-keypair-generator`. Keeping this tool to the raw pubkey point avoids duplicating those. |
| Mnemonic-phrase input | SecretScan | **out of scope** — covered by the BIP39 / `hd-key-derive` family; a mnemonic → seed → child key path is a different tool. |
| Random keypair generation | (adjacent tools) | **out of scope** — this tool derives from a *given* key; `crypto-keypair-generator` / `ecdsa-secp256k1` generate random ones. |

## Positioning vs existing gizza blocks (dup check)
- `ecdsa-secp256k1` `generate` only produces **random** keypairs; its `key` param feeds
  sign/verify. It has no "derive the public key of THIS private key" operation → gap.
- `crypto-keypair-generator` generates random keys only.
- `bitcoin-address` / `hd-key-derive` output **addresses** (base58/bech32), not the raw
  public-key point in both SEC1 forms + coordinates.
- Therefore a focused private-key → public-key-point tool is a genuine, non-duplicate gap.

All copy, examples, and design are original; no competitor text, branding, or trademarks were
copied.

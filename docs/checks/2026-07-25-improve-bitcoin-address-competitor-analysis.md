# bitcoin-address — competitor analysis (2026-07-25)

Tool function: derive Bitcoin address formats and WIF from a secp256k1 private key.

Web search performed: `bitcoin address generator private key WIF P2PKH P2WPKH online tool`.

## Competitors skimmed

1. Browser Bitcoin address generators that create or accept a private key and show P2PKH, P2SH/SegWit, and Bech32 addresses.
2. Wallet/address utilities that derive mainnet/testnet addresses from WIF and display public key, HASH160, and WIF.
3. SegWit-focused key tools that accept WIF and output legacy, wrapped SegWit, and native SegWit addresses.

## Table-stakes decisions

| Capability | Competitor pattern | Decision |
| --- | --- | --- |
| Hex private-key input | Common for test vectors and developer tools | In model: required `key`, accepts 64 hex chars with optional `0x`, spaces, underscores |
| WIF input | Common wallet/user format | In model: decode Base58Check WIF, auto-detect network and compression |
| Mainnet/testnet | Expected on address tools | In model: `network` enum for hex input; WIF overrides it |
| Compressed/uncompressed public key | Expected because legacy addresses differ | In model: boolean `compressed`; WIF overrides it |
| P2PKH legacy address | Table-stakes | In model: Base58Check version 0x00/0x6f |
| P2SH-P2WPKH wrapped SegWit | Table-stakes on SegWit tools | In model for compressed keys |
| P2WPKH native SegWit | Table-stakes | In model: Bech32 v0 with bc/tb HRP |
| WIF output | Table-stakes | In model: Base58Check with compressed flag when appropriate |
| Generate random key | Some tools do this | Out of scope/duplicate: existing `crypto-keypair-generator` generates fresh wallet keypairs |
| BIP32 derivation paths | HD-wallet tools do this | Out of scope/duplicate: existing `hd-key-derive` derives BIP32 child keys and addresses |
| QR codes / balance lookup | Some wallet tools include these | Out of model: network lookups and visual extras not needed for this derivation utility |

## Verified example target

Private key `1` (hex `000…001`) is the standard secp256k1 generator-point vector. Mainnet compressed output should include:

- P2PKH `1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH`
- WIF `KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn`
- a `bc1q…` native SegWit address

No competitor copy or branding reused.

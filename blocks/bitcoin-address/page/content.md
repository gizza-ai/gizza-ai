## What this tool does

Paste an existing Bitcoin secp256k1 private key and derive the standard address
formats and WIF export locally. The tool accepts either a 32-byte hex private key
or a WIF string. For hex input you choose mainnet or testnet and whether the
public key should be compressed. For WIF input, the embedded network and
compression flag are detected from the WIF itself.

The output includes the private key hex, WIF, public key hex, HASH160, legacy
P2PKH address, wrapped SegWit P2SH-P2WPKH address, and native SegWit P2WPKH
address when the key is compressed.

## Address formats

| Format | Mainnet prefix | Testnet prefix | Notes |
| --- | --- | --- | --- |
| P2PKH legacy | `1…` | `m…` or `n…` | Hash160 of the selected public-key encoding, Base58Check |
| P2SH-P2WPKH wrapped SegWit | `3…` | `2…` | Hash160 of the v0 witness redeem script, Base58Check |
| P2WPKH native SegWit | `bc1q…` | `tb1q…` | Bech32 witness v0 program |
| WIF | `K…`/`L…` or `5…` | `c…` or `9…` | Base58Check private-key export |

SegWit address formats require compressed public keys. If you derive from an
uncompressed key, the tool returns the P2PKH address and WIF, and explains why
P2SH-P2WPKH/P2WPKH are not emitted.

## Example

Private key `0000000000000000000000000000000000000000000000000000000000000001`
with mainnet + compressed produces:

- P2PKH: `1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH`
- WIF: `KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn`
- Native SegWit: a `bc1q…` P2WPKH address

## FAQ

<details>
<summary>Does this generate a new Bitcoin private key?</summary>

No. It derives addresses from a private key you already provide. If you need a
fresh random wallet keypair, use the wallet keypair generator instead.

</details>

<details>
<summary>Is my private key uploaded?</summary>

No. The derivation runs locally in your browser or CLI using pure Rust code. Treat
private keys carefully anyway: avoid pasting real production funds into any tool
you do not control.

</details>

<details>
<summary>Why are SegWit addresses missing for an uncompressed key?</summary>

P2SH-P2WPKH and P2WPKH spend paths are defined for compressed public keys. For an
uncompressed key the tool returns the legacy P2PKH address and WIF, then marks the
SegWit fields as requiring a compressed key.

</details>

<details>
<summary>Can I use WIF input?</summary>

Yes. WIF is decoded with Base58Check. Its version byte selects mainnet or testnet,
and its optional compression flag selects compressed or uncompressed public-key
handling, overriding the page controls.

</details>

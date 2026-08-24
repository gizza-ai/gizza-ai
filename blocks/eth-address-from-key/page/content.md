## About this tool

Ethereum account addresses are derived from secp256k1 public keys. Starting with a private key, the public key is first computed on the secp256k1 curve. Starting with a public key, the key is decoded from compressed SEC1, uncompressed SEC1, or raw `x || y` bytes. The Ethereum address is then the last 20 bytes of `Keccak-256(uncompressed_public_key_without_the_04_prefix)`.

The checksum form is EIP-55: take the lowercase 40-hex-character address, hash those ASCII hex characters with Keccak-256, and uppercase each address nibble whose matching hash nibble is 8 or higher. For the canonical private key `0000000000000000000000000000000000000000000000000000000000000001`, the checksum address is:

```
0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf
```

Choose **All fields** if you want the checksum address plus the lowercase address, bare no-prefix form, and the compressed/uncompressed public keys. Choose **JSON** when piping the output into another script.

Limits and edge cases: this is a deterministic address derivation tool, not a wallet. It does not generate entropy, sign transactions, query balances, validate ownership, or accept mnemonics, keystore JSON, WIF, or xpub values. Private keys must be exactly 32 bytes and valid secp256k1 scalars. Public keys must be valid curve points encoded as 33-byte compressed SEC1, 65-byte uncompressed SEC1, or 64-byte raw `x || y` hex. A leading `0x` is accepted, and whitespace, underscores, colons, and hyphens are ignored for paste-friendly formatting.

## FAQ

<details>
<summary>Does this send my private key anywhere?</summary>

No. The page runs the same WebAssembly code in your browser that the CLI runs locally. The key is used only to derive the public key and address; nothing is uploaded, stored, or looked up on-chain.

</details>

<details>
<summary>Can I paste a public key instead of a private key?</summary>

Yes. Use **Auto-detect** or **Public key** and paste a compressed public key beginning with `02` or `03`, an uncompressed SEC1 public key beginning with `04`, or the raw 64-byte `x || y` public-key coordinates. All three forms derive the same Ethereum address when they describe the same point.

</details>

<details>
<summary>Why is the checksum address mixed-case?</summary>

EIP-55 encodes a checksum in the letter casing of the hexadecimal address. Wallets can use that casing to catch many mistyped addresses while preserving compatibility with systems that compare addresses case-insensitively after lowercasing.

</details>

<details>
<summary>Is this the same as a Bitcoin address from the same key?</summary>

No. The secp256k1 public key point is the same, but Ethereum uses Keccak-256 and keeps the last 20 bytes directly as a `0x` address. Bitcoin uses HASH160 plus network/version bytes and Base58Check or Bech32 encoding. Use a Bitcoin-specific address tool for those formats.

</details>

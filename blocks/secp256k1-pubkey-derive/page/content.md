## About this tool

Every secp256k1 private key has exactly one public key. This tool computes it by
multiplying the secp256k1 generator point **G** by your private key scalar —
`public_key = private_key · G` — and shows the resulting point in every encoding
you're likely to need.

Paste a private key as **64 hex characters** (a leading `0x`, spaces, and
underscores are fine) or as a **WIF** (Wallet Import Format, base58check — the
`5…`/`K…`/`L…` mainnet or `9…`/`c…` testnet strings that wallets export). You get
back:

- **Compressed** SEC1 point — 33 bytes, `02` or `03` prefix followed by the X
  coordinate. The prefix records whether Y is even (`02`) or odd (`03`), so the
  full point is recoverable from X alone.
- **Uncompressed** SEC1 point — 65 bytes, `04` prefix followed by X then Y.
- **X coordinate** — the raw 32-byte X value. This is also the *x-only* public
  key used by Bitcoin Taproot (BIP340).
- **Y coordinate** — the raw 32-byte Y value.
- **Y parity** — `even` or `odd`, matching the compressed prefix.

The public-key *point* is the same no matter which chain you use it on
(Bitcoin, Ethereum, Tron, and others all share secp256k1) — only the later
address encoding differs.

### Worked example

Private key `0000…0001` (the number **1**) derives the secp256k1 generator point:

```
compressed: 0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
uncompressed: 0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
x: 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
y: 483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
y_parity: even
```

Choose a single **Output** option to get just that value as bare hex (handy for
copy-paste into a script or wallet import).

### Limits and privacy

- Derivation happens entirely in your browser via WebAssembly — the private key
  is never uploaded anywhere.
- The private key must be a valid secp256k1 scalar: non-zero and below the curve
  order. `0000…0000` and out-of-range values are rejected.
- This tool derives from a key you already have — it does not generate random
  keys, and it does not produce Bitcoin/Ethereum addresses (those are separate
  steps that hash or encode the public key shown here).

## FAQ

<details>
<summary>What's the difference between the compressed and uncompressed public key?</summary>

Both encode the same elliptic-curve point. The **uncompressed** form (`04` + X +
Y) stores both coordinates: 65 bytes. The **compressed** form (`02`/`03` + X)
stores only X plus a one-byte parity flag for Y: 33 bytes. Because the curve is
symmetric about the X axis, each X has just two possible Y values, so the parity
byte is enough to reconstruct Y. Modern Bitcoin uses compressed keys to save
space.

</details>

<details>
<summary>Why do compressed keys start with 02 or 03?</summary>

The prefix encodes the parity of the Y coordinate: `02` means Y is even, `03`
means Y is odd. Together with the X coordinate that follows, this uniquely
identifies the point. The `y_parity` field in the "all" output tells you which
one applies.

</details>

<details>
<summary>Can I paste a WIF private key?</summary>

Yes. Paste a base58check WIF (the `5…`/`K…`/`L…` mainnet or `9…`/`c…` testnet
string a wallet exports) and the tool decodes the underlying 32-byte scalar. The
WIF's network byte and compression flag don't change the public-key point, so
both the compressed and uncompressed forms are always shown regardless of the
flag.

</details>

<details>
<summary>Is the X coordinate the same as a Taproot (x-only) public key?</summary>

Yes. Bitcoin Taproot (BIP340) uses x-only public keys — just the 32-byte X
coordinate, with no prefix and no explicit Y. The **X coordinate** output is
exactly that value.

</details>

<details>
<summary>Does this give me a Bitcoin or Ethereum address?</summary>

No — it stops at the public-key point. An address is a further step: Bitcoin
hashes the compressed key (SHA-256 then RIPEMD-160) and base58check/bech32-encodes
it; Ethereum takes the Keccak-256 of the 64-byte X‖Y and keeps the last 20 bytes.
This tool focuses on deriving the raw public key those steps start from.

</details>

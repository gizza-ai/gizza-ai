# eth-address-from-key — competitor analysis (2026-08-22)

Paraphrased scan notes for `blocks/eth-address-from-key`; no competitor copy, branding, or trademark text is reused.

## Competitors scanned

1. Reference Ethereum documentation and EIP-55 examples: address = last 20 bytes of Keccak-256 over the uncompressed secp256k1 public key without the `04` prefix; checksum casing comes from Keccak-256 over lowercase address hex.
2. Browser wallet/address derivation snippets and calculators: common controls are private key input, public key input, output address, and checksum/lowercase display. Many also generate random keys, which is not this tool's purpose.
3. General crypto conversion suites: often expose secp256k1 public-key derivation and Keccak hashing as separate steps, but not one paste-to-address flow; they usually support compressed/uncompressed public keys and `0x` prefixes.
4. Wallet libraries and CLIs: derive addresses from private keys and public keys, but include network/RPC, signing, mnemonics, keystores, or HD paths that are outside a pure deterministic block.

## Table stakes and decisions

| Capability | Decision |
| --- | --- |
| Private key to address | Built: 32-byte secp256k1 private-key hex derives the public key then the address. |
| Public key to address | Built: accepts compressed SEC1, uncompressed SEC1, and raw x||y public keys. |
| EIP-55 checksum | Built: checksum address is the default prominent output and separately selectable. |
| Lowercase and bare forms | Built: lowercase `0x...` and no-prefix 40-hex outputs. |
| `0x` and formatted paste tolerance | Built: leading `0x` plus whitespace, underscores, colons, and hyphens are ignored. |
| JSON / scriptable output | Built for CLI and page workflows. |
| Key generation | Out of model for this tool; generating private keys is a wallet/keygen workflow and should not be mixed into a deterministic derivation utility. |
| Mnemonics, keystore JSON, HD paths | Out of model; they require wallet-specific KDF/path semantics and are covered by different tool classes. |
| Chain/RPC lookup or balance | Out of model; this repo's pure blocks should not depend on network calls for address derivation. |

## UX decisions

- Keep `key_type=auto` as default because byte length unambiguously separates 32-byte private keys from 33/64/65-byte public keys.
- Keep `output_format=all` as default so users can see checksum, lowercase, bare address, and public-key material in one run.
- Do not add a password field, keystore decryptor, or random-key generator; those would imply wallet storage/security semantics that this deterministic address derivation block does not provide.

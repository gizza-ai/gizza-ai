# crypto-keypair-generator — competitor analysis (2026-07-17)

**Tool:** generate a fresh keypair + wallet address for a blockchain (Bitcoin, Ethereum, Solana), fully offline. secp256k1 (`k256`) for BTC/ETH, Ed25519 (`ed25519-dalek`) for SOL. Surfaces: chat + CLI (no page — non-deterministic generation doesn't fit the page's recompute-on-input model).

## Verified surfaces
- **CLI:** `gizza tool crypto-keypair-generator chain=bitcoin|ethereum|solana` — all three produce structurally-correct output (BTC `1…` P2PKH + `L…` compressed WIF; ETH EIP-55 mixed-case + `0x04` uncompressed pubkey; SOL base58 address + 64-byte keypair export).
- **Correctness (known-answer tests):** EIP-55 spec vectors; Ethereum private-key=1 → `0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf` (proves k256 secp256k1 pubkey + Keccak-256 + EIP-55); base58check all-zero P2PKH vector; Bitcoin generator-point structure; Solana decode round-trip.
- **Page:** none (by design, like `generate-ecdsa-key-pair` / `ed25519-key-pair-generator`).

## Top competitors
1. **WalletGenerator.com** — client-side paper-wallet generator, many coins, QR codes, print layout.
2. **cryptowallet-cli (`cw`)** — CLI generator across many chains (BTC/ETH/EVM/Solana/SUI/TON…).
3. **btc-vanity (Rust)** — fast BTC/ETH/SOL **vanity** address generator (pattern matching).
4. **TheHackerWire / Mitilena / TokenPocket generators** — multi-chain browser generators with **BIP39 seed phrases** + HD derivation.
5. **Solana-keypair-generator** — single-chain offline Solana keypair generator.

## Gap analysis (fit-to-model)
| Competitor capability | Status |
|---|---|
| Fully offline / local generation | ✅ have it (core value; runs in-sandbox, nothing leaves the device) |
| BTC legacy address + WIF | ✅ |
| ETH EIP-55 address | ✅ |
| SOL base58 keypair | ✅ |
| **BIP39 mnemonic + HD (BIP32/44) derivation** | out of scope — already covered by sibling tools `bip39-mnemonic-generator` + `bip39-seed-derive`; adding here would duplicate |
| **Vanity addresses** (pattern match) | out of model — a distinct compute-loop tool, not a keypair generator |
| **QR codes / printable paper wallet** | out of model — needs page/image render output; this is a text chat/CLI tool with no page |
| More chains (BNB/LTC/DOGE, etc.) | deferred — BNB is an EVM clone of the ETH address (redundant); LTC/DOGE are base58check version-byte variants. Not added: each new chain is a crypto-correctness surface, and the three shipped chains already cover the three *distinct* address families. Additive later if demanded. |
| SegWit / bech32 BTC addresses (`bc1…`) | deferred — a second BTC address type; legacy P2PKH is the canonical/most-compatible default. Additive later. |

## Decision
No in-model gaps warrant changes: the tool covers the three distinct address families with verified-correct derivation, is fully offline, and complements (rather than duplicates) the existing `bip39-*`, `generate-ecdsa-key-pair`, `ed25519-key-pair-generator`, and `generate-rsa/pgp-key-pair` tools. The competitor differentiators are either already covered by sibling tools (BIP39/HD) or out of the chat/CLI model (vanity, QR/paper). Kept focused for crypto correctness. No competitor copy/branding was used.

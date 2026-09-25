# Competitor analysis: eth-vanity-address

Date: 2026-08-29
Tool: `eth-vanity-address`
Backlog description: Grinds for an Ethereum keypair whose address matches a desired prefix/suffix for vanity or testing.

## Sources scanned

Search query: `Ethereum vanity address generator prefix suffix case-sensitive estimate attempts tool`

- CreateMyToken Ethereum Vanity Address Generator: web UI advertises a hex-only selector, case-sensitive toggle, prefix/suffix mode, worker thread setting, start/stop status, example address, difficulty, and speed reporting.
- OpenSource Toolkit EVM Vanity Address Generator: web UI advertises EVM vanity addresses with specific patterns, prefix/suffix matching, real-time statistics, difficulty controls, and client-side processing.
- Axelbase ETH Vanity Generator: web UI documents 1-8 hex-character prefix input, validation for `0-9`/`a-f`, examples such as `cafe`, `1337`, `dead`, and `beef`, plus difficulty warnings and estimated average attempts.
- Leutenegger `vanity-eth` CLI: command-line examples cover Ethereum prefix, suffix, thread counts, exact case matching/EIP-55 checksum, and substring matching; it also supports Bitcoin address families.

## Table-stakes capabilities and decisions

| Capability / UX pattern | Seen in competitors | Decision for this block |
| --- | --- | --- |
| Hex prefix matching after `0x` | Common across web and CLI tools | In-model; implemented as `prefix`, with validation and leading `0x` stripping. |
| Hex suffix matching | CreateMyToken, OpenSource Toolkit, CLI tools | In-model; implemented as `suffix`, combinable with prefix. |
| Case-sensitive / EIP-55 matching | CreateMyToken, Vanity-ETH pages, `vanity-eth` CLI | In-model; implemented as `match_case`, matching the checksummed address and reflecting the extra difficulty. |
| Difficulty / expected-attempt reporting | CreateMyToken, Axelbase, OpenSource Toolkit | In-model; implemented in normal miss messages and `estimate` output mode. |
| Start/stop live progress and speed counters | Browser generators with workers | Out-of-model for this synchronous gizza block; the block returns one final result. Attempt cap and estimate mode keep runs bounded. |
| Worker thread / GPU acceleration | CreateMyToken worker controls, standalone vanity tools | Out-of-model; gizza blocks run as portable Rust/WASM without GPU workers. |
| Reproducible seed for demos/tests | Useful for deterministic examples | In-model; implemented as optional `seed`. Blank seed uses platform CSPRNG. |
| Output only address / only private key / JSON | CLI-style tools expose focused outputs | In-model; implemented as `output_format` enum. |
| Substring-anywhere matching | `vanity-eth` CLI supports contains | Out-of-model for this first block to keep descriptor simple; prefix/suffix are the common web table stakes. |
| Non-Ethereum chains / Bitcoin address families | `vanity-eth` CLI | Out-of-model; this tool is Ethereum/EVM address derivation only. |
| UX presets / examples | Axelbase examples and generator preset-like examples | In-model; page includes example chips for estimating, deterministic prefix search, and case-sensitive JSON. |
| Attempt limit control | Difficulty controls / worker-oriented UIs | In-model; `max_attempts` uses a bounded numeric control and page slider metadata. |

## Resulting design

The gizza tool ships a local, deterministic-friendly Ethereum vanity address grinder with prefix, suffix, case-sensitive checksum matching, max-attempt cap, reproducible seed, and selectable output format. It intentionally does not promise live progress, worker/GPU acceleration, balance checks, transaction signing, mnemonic/keyfile handling, substring-anywhere matching, or non-Ethereum address families.

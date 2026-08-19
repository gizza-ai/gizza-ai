# Competitor analysis: xpub-derive

Date: 2026-08-15
Tool: `xpub-derive`
Backlog request: Derive receive and change addresses from an extended public key for watch-only inspection.

## Sources skimmed

Web search and existing wallet-tool patterns were used to compare against common xpub/address derivation surfaces:

1. Ian Coleman-style BIP39/BIP32 derivation pages — seed/mnemonic roots, derivation path controls, address table output, BIP44/49/84 presets.
2. Wallet explorer / xpub address tools — paste an xpub/ypub/zpub, select external/change chain, list addresses with indexes for watch-only checking.
3. Hardware-wallet/watch-only wallet flows (Sparrow/Electrum style) — account-level extended public key, receive/change branches, gap-limit batches, and address type implied by script policy.

No competitor copy, branding, or UI text was reused.

## Table-stakes capabilities and decisions

| Capability / UX pattern | Competitor pattern | In model? | Decision for gizza |
| --- | --- | --- | --- |
| Paste an extended public key | Dedicated xpub tools and watch-only wallets start from an account-level xpub. | Yes | Required `xpub` string input; private prefixes are rejected. |
| Mainnet/testnet support | Tools commonly support xpub/tpub and newer script prefixes. | Yes | Prefix classification for xpub/ypub/zpub and tpub/upub/vpub. |
| Address type selection | Some tools infer from prefix, others offer legacy/SegWit controls. | Yes | `address_type=auto` follows the prefix; explicit enum can override. |
| Receive/change branches | Watch-only wallets show external receive and internal change paths. | Yes | `chain=receive|change|both`, deriving `m/0/i` and/or `m/1/i`. |
| Gap-limit batches | Wallet inspection commonly checks 20-address ranges. | Yes | `count` 1–100, default 10, example chip for 20-address CSV export. |
| Start index / paging | Address explorers let users inspect later ranges. | Yes | `start` parameter with non-hardened BIP32 range checks. |
| CSV/export shape | Audit workflows often copy rows into a spreadsheet. | Yes | `format=table|csv|list` plus optional public-key column. |
| Public-key visibility | Developer/debug tools show compressed public keys. | Yes | `include_public_key` checkbox for table/CSV outputs. |
| Hardened child derivation | Cannot be done from public keys. | Out of model | Rejects ranges beyond non-hardened indexes; docs explain limitation. |
| Mnemonic/seed/xprv support | Broad BIP39 tools derive from private material. | Out of model for this tool | Existing `hd-key-derive` covers private derivation; this tool stays watch-only and rejects private keys. |
| Multisig descriptors / Taproot policies | Modern wallets use descriptors and x-only Taproot keys. | Out of model | Single-signature BIP32 address derivation only; multisig `Ypub`/`Zpub`, descriptors, and Taproot are documented limits. |

## Implementation shape

The tool uses pure-Rust BIP32 public derivation and the existing Bitcoin address encoding patterns already proven in `hd-key-derive` and `bitcoin-address`. It exposes fixed-choice enums so the page renders selects, a bounded slider for the count, and example chips for common BIP84/gap-limit/testnet scenarios.

Descriptor parameters:

- `xpub` (required)
- `chain`: `receive`, `change`, `both`
- `count`: integer 1–100
- `start`: integer 0–2147483647
- `address_type`: `auto`, `p2pkh`, `p2sh_p2wpkh`, `p2wpkh`
- `format`: `table`, `csv`, `list`
- `include_public_key`: boolean

## Limits documented for users

- Does not accept or derive from private extended keys.
- Cannot derive hardened children from an xpub.
- Single-signature Bitcoin address formats only.
- Multisig descriptors, Taproot x-only addresses, and altcoin version bytes are not implemented.

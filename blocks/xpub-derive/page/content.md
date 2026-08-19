# Watch-only xpub address derivation

Derive Bitcoin receive and change addresses from an extended public key without ever handling private keys. Paste an `xpub`, `ypub`, `zpub`, `tpub`, `upub`, or `vpub`, choose the chain and range, and get deterministic watch-only addresses for audits, wallet checks, and gap-limit review.

Everything runs locally in the browser. Extended private keys are rejected.

## Worked example

Use the BIP84 account zpub from the public test vector:

- `xpub`: `zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs`
- `chain`: `receive`
- `count`: `2`
- `start`: `0`
- `address_type`: `auto`
- `format`: `table`

The first two receive addresses are `bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu` and `bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g`.

## Inputs

- `xpub` accepts single-signature extended public keys: `xpub`/`ypub`/`zpub` on mainnet and `tpub`/`upub`/`vpub` on testnet.
- `chain` chooses receive (`m/0/i`), change (`m/1/i`), or both.
- `count` derives 1 to 100 addresses per selected chain. `20` matches the common wallet gap-limit batch.
- `start` pages through a chain without deriving hardened children.
- `address_type=auto` follows the key prefix; explicit choices can render legacy, wrapped SegWit, or native SegWit addresses from the same public key.
- `format` selects a readable table, CSV rows, or a bare address list.

## Limits and edge cases

This is a Bitcoin single-signature BIP32/BIP44-style public derivation tool. It does not derive hardened children from an xpub, does not accept private keys or mnemonics, and does not support multisig `Ypub`/`Zpub` policy descriptors. Taproot x-only output, script descriptors, and altcoin version bytes are outside this tool's model.

<details>
<summary>Is it safe to paste an xpub here?</summary>

An xpub is public enough to derive addresses, but it can reveal wallet history if shared. This page runs locally and never uploads it, but you should still treat extended public keys as sensitive watch-only wallet data.
</details>

<details>
<summary>Why are xprv, yprv, and zprv rejected?</summary>

Those are extended private keys. A watch-only address tool should not need private material, so this tool rejects private prefixes instead of deriving from them.
</details>

<details>
<summary>What does address_type=auto do?</summary>

`auto` follows the extended key prefix: `xpub`/`tpub` produce legacy P2PKH addresses, `ypub`/`upub` produce wrapped SegWit P2SH-P2WPKH addresses, and `zpub`/`vpub` produce native SegWit bech32 addresses.
</details>

<details>
<summary>How do I check the usual wallet gap limit?</summary>

Set `chain=receive`, `start=0`, and `count=20` to list the first 20 receive addresses. Increase `start` by 20 for the next page.
</details>

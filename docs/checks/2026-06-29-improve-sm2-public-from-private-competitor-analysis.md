# sm2-public-from-private — competitor analysis & differentiation

**Tool:** `gizza-ai/sm2-public-from-private` — derive the SM2 public key from a
private key (Chinese national standard GM/T 0003, OSCCA curve `sm2p256v1`):
compute `Q = d·G` and serialise it in standard encodings.
**Date:** 2026-06-29

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `openssl pkey -in priv.pem -pubout` (SM2-enabled build) | CLI | The reference, but only consumes a PKCS#8/SEC1 **PEM** — there's no path from a bare 32-byte hex scalar without first hand-assembling a key file; getting the public point as raw hex (compressed/uncompressed) is a separate `-text`/`-pubin -text` parse step; needs a recent OpenSSL (≥1.1.1 / 3.x) built with SM2. |
| GmSSL CLI / Tongsuo (BabaSSL) | CLI | Authoritative SM2 toolkits, but a heavyweight install most people don't have; PEM/DER-centric, no one-shot "scalar in → every encoding out". |
| Online "SM2 private→public" sites | Web | Several **send the private key to a server** to derive the point — a non-starter for key material. Many are Chinese-language only; most emit a single encoding (raw hex point *or* PEM, rarely both, rarely compressed). |
| `gmssl`/`sm-crypto` (JS), `gmssl` (Go), `cryptography`+`gmssl` (Python) | Library | Require writing code; each picks its own input convention (raw hex vs PKCS#8 vs DER) and its own output encoding, so interop needs manual point (de)serialisation. |

## How gizza's tool is better / different

1. **Runs locally — the private key never touches a server.** The derivation is
   pure WASM in the chat service worker, the CLI, or the browser page; the private
   key is used only to compute the public point and is **never echoed back** —
   only public key material is returned. The single most important property here,
   and where most web competitors fail.
2. **Takes the key in the form you actually have it.** Accepts a raw 32-byte
   **hex scalar** (optional `0x`, embedded whitespace tolerated) *or* a **PKCS#8
   PEM**, with `input_format=auto` detecting which — no need to wrap a bare scalar
   in a key file just to run OpenSSL.
3. **Every public encoding at once.** One call returns **SPKI PEM**, **SEC1
   uncompressed** hex (`04 || x || y`, 130 chars), **SEC1 compressed** hex
   (`02|03 || x`, 66 chars), and the affine **x**/**y** coordinates — pick `all`
   for the labelled summary or one specific encoding. Drops straight into OpenSSL,
   GmSSL, or any language lib without conversion.
4. **Deterministic & standards-conformant.** `Q = d·G` is one scalar
   multiplication — the same private key always yields the same public key (no
   RNG). The core tests pin the GM/T 0003 worked example (scalar `3945…C5B8` →
   the standard `(x, y)`), check the compressed-prefix parity, and confirm the
   hex and PEM input paths agree.
5. **No toolchain to install.** Unlike `openssl`/GmSSL/Tongsuo (which need a
   build with SM2 enabled), this works from chat, a single `gizza tool` call, or
   the browser page.

## Surfaces & honest scope

- **Chat + CLI + page** — unlike the sibling `sm2-keypair-generate` (a
  zero-input, non-deterministic generator with no page), this tool is
  deterministic and input-driven, so it fits the page's recompute-on-input model.
  The page renders `private_key` as a textarea and `input_format` / `output_format`
  as `<select>`s (options single-sourced from the descriptor enum), and supports
  query-param deep links (e.g. `?private_key=…&output_format=compressed`).

## Possible future enhancements

- Optional DER (binary) output alongside PEM.
- Accept a SEC1 (`-----BEGIN EC PRIVATE KEY-----`) PEM in addition to PKCS#8.
- Sibling `sm2-sign` / `sm2-verify` / `sm2-encrypt` tools (the RustCrypto `sm2`
  crate exposes `dsa` signing and `pke` encryption) pairing with this and the
  existing `ecdsa-sign` / `rsa-sign` family.

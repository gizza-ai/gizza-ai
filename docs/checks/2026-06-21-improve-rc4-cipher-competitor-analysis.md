# rc4-cipher — competitor analysis (2026-06-21)

Tool: `blocks/rc4-cipher` — encrypt/decrypt text with the RC4 stream cipher, with an
optional drop-N (RC4-drop) to skip the biased initial keystream bytes; hex/base64 I/O.

## Surfaces verified
- **Chat block:** `wafer build` instantiates the wasm32-wasip1 block (316 KiB), validates OK.
- **CLI:** `gizza tool rc4-cipher` — encrypt of `Key`/`Plaintext` yields the canonical
  vector `bbf316e8d940af0ad3`; decrypt round-trips; `drop=768` round-trips; encoded-key +
  base64 output work.
- **Page:** `/tools/rc4-cipher/` — Playwright (3 specs): encrypt→decrypt round-trip, the
  classic Key/Plaintext vector, and the empty-key error path all pass.
- **Drift guard:** `schema_json_matches_authored_chat_schema` unit test passes — no
  LLM-facing schema drift.

## Competitors reviewed (top general-purpose RC4 web tools)

1. **CyberChef "RC4" operation** (GCHQ). Key as UTF8/Hex/Base64/Latin1; input/output as
   Hex/Base64/Latin1/UTF8. No explicit drop-N op (it has a separate "RC4 Drop" operation).
   Part of a recipe pipeline.
2. **CyberChef "RC4 Drop" operation.** Adds the `drop` byte count (number of keystream
   bytes to discard) — exactly the drop-N feature.
3. **dCode "RC4 Cipher".** Plaintext + key; output hex/base64; encrypt and decrypt.
   No drop-N. Pedagogical explainer of the KSA/PRGA.
4. **Online tools (e.g. devglan / cryptii "RC4").** Text + key, encrypt/decrypt, hex/base64.
   Single fixed key interpretation (usually UTF-8). No drop-N.
5. **emn178 / md5calc RC4 tools.** Text + key → hex. Encrypt only on some; minimal options.

## Gap analysis (fit-to-model)

| Capability | Competitors | rc4-cipher | Status |
|---|---|---|---|
| Encrypt + decrypt | most | yes (symmetric) | covered |
| Key as UTF-8 passphrase | all | `key_format=text` (default) | covered |
| Key as hex/base64 bytes | CyberChef | `key_format=encoded` | covered |
| Ciphertext hex output | all | `format=hex` (default) | covered |
| Ciphertext base64 output | most | `format=base64` | covered |
| Drop-N (RC4-drop[n]) | CyberChef only | `drop` param (0..n) | covered — matches CyberChef "RC4 Drop" |
| Canonical test vectors | dCode docs | unit + CLI + page assert `BBF316E8D940AF0AD3` | covered |
| Key length validation (1–256 B) | partial | explicit error | covered (better than most) |
| In-browser / no upload | cryptii | yes (wasm) | covered |
| Latin1 / raw-byte I/O of ciphertext | CyberChef | not offered | out of scope — hex/base64 cover binary-safe round-trips; raw Latin1 adds little for a JSON/CLI tool |
| Recipe chaining | CyberChef | n/a | out of scope (different product shape) |

## Decisions
- Implemented drop-N from the start (the row's headline feature; only CyberChef offers it),
  so rc4-cipher matches the most capable competitor without copying any UI/branding.
- Both key interpretations (text passphrase / encoded bytes) and both output encodings
  (hex/base64) are supported, matching CyberChef's flexibility.
- Prominent security warning on the page, chat skill, and CLI description: RC4 is broken;
  pointer to `aes-cipher` / `text-encrypt` for real use. No competitor copy/branding reused.
- No remaining in-model gaps. Latin1/raw-byte ciphertext I/O is intentionally omitted as
  low-value for a JSON-result tool (hex/base64 already binary-safe).

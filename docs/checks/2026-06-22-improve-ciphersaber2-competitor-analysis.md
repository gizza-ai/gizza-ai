# ciphersaber2 — competitor analysis (2026-06-22)

Tool: **CipherSaber-2** — encrypt/decrypt text with the CipherSaber cipher: RC4
(ARCFOUR) keyed by `user_key || random 10-byte IV`, with RC4's key-scheduling
loop repeated `rounds` times (the spec recommends 20, defending against the FMS
attack that breaks CipherSaber-1). The IV is prepended to the ciphertext in the
clear and read back on decrypt. Pure Rust (RC4 hand-rolled + `getrandom` for the
IV) → runs on all surfaces (chat, CLI, standalone page).

## Surfaces verified

- **chat block** — `wafer build` validates `target/block.wasm` instantiates
  (321.7 KiB); `getrandom` resolves to wasi `random_get`. Schema drift guard test
  passes.
- **CLI** — `gizza tool ciphersaber2 data="Attack at dawn" key=asecretkey rounds=20
  iv=0102030405060708090a operation=encrypt` →
  `0102030405060708090a937fd7024cb2ceb9efbee3066475` (IV prepended in the clear);
  decrypting that with the same key+rounds returns `Attack at dawn`. Random-IV
  encrypt + roundtrip, base64 + UTF-8 (`привет`) at rounds=1, and the bad-IV-length
  error path all verified.
- **page** — 3 Playwright tests pass: fixed-IV vector then decrypt, random-IV
  roundtrip (browser crypto RNG via the `getrandom` `js` feature), empty-key error.

## Top competitors surveyed

1. **BartMassey/ciphersaber2 (Haskell)** + the Hackage `ciphersaber2` library —
   reference CLI implementation; N-round key setup, 10-byte IV. CLI/library only.
2. **JVMartin/ciphersaber-2 (C)** — file encrypt/decrypt CLI; reads/writes the
   IV at the head of the file. No GUI.
3. **RJ-Russell/Ciphersaber-2 (Python)** — chat program embedding CS-2 RC4.
4. **AutoIt CipherSaber-2 example** — desktop-script implementation.
5. **Generic "RC4 online" web tools** (rc4.online, devglan-style) — plain RC4 with
   no IV and no repeated key setup; NOT CipherSaber (no interop with CS-2 files).

Finding: CipherSaber-2 is widely implemented as CLI/library code, but there is
**no well-known browser tool** that does CS-2 with the random IV + rounds and
hex/base64 I/O. This page fills that gap.

## Gap diff and ranking (fit-to-model)

| Capability | Competitors | This tool | Action |
|---|---|---|---|
| Correct CS-2 construction (key‖IV, N-round KSA) | reference CLIs yes | yes (default rounds=20) | covered |
| Auto random 10-byte IV, prepended in clear | yes (CLIs) | yes (`getrandom`) | covered |
| Read IV back from ciphertext on decrypt | yes | yes (first 10 bytes) | covered |
| Configurable rounds incl. N=1 (CipherSaber-1) | partial | yes (`rounds`, min 1) | **covered, ahead of GUI tools** |
| Explicit IV for deterministic/interop output | rare | yes (`iv`, encoded 10 bytes) | **differentiator** |
| Text **or** encoded (hex/base64) key | rare in GUIs | yes (`key_format`) | covered |
| hex / base64 ciphertext encoding | mixed (raw files common) | both | covered |
| UTF-8 plaintext (emoji, Cyrillic) | varies | yes | covered |
| Local / no-upload privacy | desktop yes, web rare | yes (in-browser wasm) | covered |
| Deep-link via query params | no | yes (page query-prefill) | covered |
| Available in chat + CLI + page | CLI only | yes (3 surfaces) | **ahead** |

### Out-of-model (NOT built — recorded, not implemented)

- **Binary file encrypt/decrypt** (the canonical CipherSaber use: encrypt a
  whole file with the IV at the head). The page input is a single text field and
  the descriptor models a text param, so file-in/file-out CS-2 would need
  `AssetKind` page wiring not part of this pure text tool. Text + hex/base64 covers
  arbitrary bytes via encoding; whole-file mode is a deliberate scope boundary
  (consistent with the sibling `rc4-cipher` / `aes-cipher` text tools).

## Copy / UX / visual

- Title/description/tags written for SEO ("ciphersaber", "ciphersaber-2", "rc4",
  "arcfour", "stream cipher"); no competitor copy/branding/trademarks copied.
- Content explains the random IV, the rounds/N=20 recommendation, the
  CipherSaber-1 vs -2 distinction, and the prominent "RC4 is broken — interop/CTF/
  learning only" caveat, cross-linking to aes-cipher / text-encrypt / encrypt-file.
- `data` is a multiline `<textarea>` so pasted multi-line content is preserved;
  `iv` placeholder makes clear blank = random.

## Not a duplicate

`rc4-cipher` is **plain RC4** with an optional drop-N and no IV — same keystream
primitive, different protocol: it has no IV, no key‖IV session key, and no
repeated key setup, so its ciphertext is not CipherSaber-compatible. CipherSaber-2
is a distinct, named cipher standard (random 10-byte IV prepended + N-round KSA).
Confirmed distinct before building; not added to the skiplist.

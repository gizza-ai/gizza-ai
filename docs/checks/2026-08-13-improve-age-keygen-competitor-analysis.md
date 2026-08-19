# age-keygen — competitor analysis (2026-08-13)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased;
no competitor copy, branding, or trademarks were reused. Out-of-model items are listed, not built.

## Duplicate check (done first)

`blocks/age-encrypt` exists and was inspected before building.

| Block | What it does | Overlap with `age-keygen` |
| --- | --- | --- |
| `age-encrypt` | Encrypts text to armored age ciphertext (passphrase or `age1…` recipients). Its skill description explicitly says *"use the age CLI for … identity generation"*. | **Consumes** `age1…` recipients; cannot produce them. Complementary, not duplicate. |
| `keypair-generator` | Generates raw X25519/Ed25519 keys as PKCS#8/SPKI PEM + hex + base64. | Same curve, **different encoding**. Emits no `age1…` / `AGE-SECRET-KEY-1…` bech32 strings, which are the only forms age clients accept. |
| `x25519-ecdh` | Derives an ECDH shared secret from an existing private + peer public key. | Consumes keys; does not generate age identities. |
| `ssh-keygen`, `generate-*-key-pair`, `crypto-keypair-generator` | SSH / RSA / ECDSA / PGP / wallet keys. | Different key formats entirely. |

**Verdict: not a duplicate — build.** The gap is concrete: a user of `age-encrypt` needs an `age1…`
recipient and nothing in the repo produces one.

## Competitors reviewed

1. **`age-keygen`** — FiloSottile/age (Go); the canonical reference implementation.
   Flags: `-o OUTPUT` (write identity to a file), `-y` (read an identity and print only its
   recipient(s), one per line, no comments), `-pq` (post-quantum hybrid ML-KEM-768 + X25519),
   `--version`. Stdout format is three lines — an RFC 3339 `# created:` comment, a
   `# public key:` comment, then the `AGE-SECRET-KEY-1…` identity. When stdout is not a terminal
   it echoes the public key to stderr.
2. **`rage-keygen`** — str4d/rage (Rust); drop-in age implementation, same file format, same
   `-o` / `-y` surface. Confirms the output format is the interop contract, not a Go detail.
3. **agewasm.marin-basic.com** — an online, Go+WebAssembly age tool. Key generation is a single
   "Generate Keys" button producing a *Private key* and *Public key* field; it makes an explicit,
   prominent claim that there is no backend and everything happens in the browser. No comment,
   seed, or output-format options.
4. **`typage` / `age-encryption` (npm)** — FiloSottile's TypeScript/browser age library:
   `generateIdentity()`, `identityToRecipient(identity)`, `generateHybridIdentity()`. Random only;
   no seeded/deterministic generation.
5. **`age-vanity-keygen`** — AlexanderYastrebov; brute-forces identities until the recipient has a
   chosen prefix, output byte-identical to `age-keygen`. Niche but a real differentiator.

## Table stakes → where each one landed

| Capability | Seen in | Fit | Where it landed |
| --- | --- | --- | --- |
| Generate an X25519 identity + its recipient | all 5 | in-model | core `generate_identity`, all surfaces |
| Canonical `# created:` / `# public key:` / secret three-line file | age-keygen, rage-keygen | in-model | `format=text` (default) + `include_created` |
| Recipient-only output (`-y`) | age-keygen, rage-keygen | in-model | `format=recipient_only` |
| Re-derive the recipient from an **existing** identity (`-y` on a key file) | age-keygen, rage-keygen | in-model | `seed` also accepts a pasted `AGE-SECRET-KEY-1…` |
| Secret-only output (no comment noise) | agewasm (separate fields) | in-model | `format=identity_only` |
| Machine-readable output | typage (returns objects) | in-model | `format=json` (extra — no CLI competitor ships it) |
| Save the identity to a file (`-o`) | age-keygen, rage-keygen | platform | page Copy button + generated Download link; CLI shell redirect |
| Explicit local-only / no-upload statement | agewasm | in-model | page hero, About, and a dedicated FAQ entry |
| Label a key so it can be told apart later | age file format allows `#` comment lines | in-model | `comment` param (single line, ≤200 chars) |
| Deterministic generation from a seed | **none** | in-model | `seed` (64-hex) — reproducible docs/test vectors; page warns it is not for real keys |

Every table stake above is either in the descriptor or in the list below. Nothing was dropped silently.

## Out of model / deliberately not built

- **`-pq` ML-KEM-768 + X25519 hybrid identities.** The `age` 0.12 crate exposes no post-quantum
  recipient type, so this cannot be produced correctly here. Named on the page as a limit.
- **Vanity recipient prefixes** (`age-vanity-keygen`). Unbounded brute-force search; a 4-character
  prefix is already ~10^6 attempts. Wrong shape for a one-shot sandboxed tool with no progress
  channel. Listed as a limit.
- **Writing the identity to a path with `0600` permissions.** No filesystem in the browser or the
  wasm sandbox; the CLI user redirects stdout instead.
- **Reading an identity file from disk/stdin.** A pure page has no file input; pasting the identity
  into `seed` covers the same `-y` workflow.
- **Passphrase-encrypting the generated identity** (`age -p key.txt`). That is `age-encrypt`'s job —
  the page cross-references it in prose rather than duplicating it.
- **SSH-key identities / age plugins.** Separate key formats; `blocks/ssh-keygen` covers SSH.

## UX control patterns adopted

- `format` is a fixed-choice `Param::enumv` → renders as a `<select>` with friendly
  `[input.labels]` (Age key file / JSON / Public recipient only / Secret key only).
- `include_created` is a `Param::boolean` default `true` → a pre-checked checkbox, matching
  age-keygen's default of always writing the timestamp.
- `[[example]]` preset chips replace the competitors' single "Generate Keys" button with several
  one-click starting points (fresh key file, recipient only, labelled key, reproducible seed).
- The generated page's Reset button re-runs generation, which is the "generate another" affordance.

## Security posture (stricter than the competitors)

- Everything runs locally — wasm in the browser tab, or the local CLI/chat sandbox. No network
  capability is declared by the block.
- The only secret material any surface ever prints is the key it just generated for the caller.
- `format=recipient_only` omits the secret key from the response entirely (not merely from the
  rendered text), so a "share my public key" run cannot leak the identity.
- The page states the local-only property and warns that a user-supplied `seed` makes the key only
  as strong as the seed, so seeds are for reproducible test vectors, not production keys.

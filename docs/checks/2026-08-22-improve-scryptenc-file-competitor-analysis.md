# scryptenc-file — competitor analysis (2026-08-22)

Scan run before/while implementing `blocks/scryptenc-file`. All notes are **paraphrased**; no
competitor copy, branding, or trademark text is reproduced or reused. Out-of-model items are
listed here only — they are not built.

## What the tool is

Read and write the **scrypt encrypted data format** (version 0): the container the Tarsnap
`scrypt enc` / `scrypt dec` utility and compatible implementations produce. 96-byte header
(`scrypt` magic, version, logN/r/p, 32-byte salt, truncated SHA-256 checksum, header
HMAC-SHA256), AES-256-CTR body at a zero nonce, trailing HMAC-SHA256 over the whole file;
scrypt emits 64 bytes split into the AES key and the HMAC key. Verified against the upstream
FORMAT description during the scan — our `core::parse_header`/`encrypt`/`decrypt` match it
field for field, and the `known_vector_is_stable` unit test pins the byte layout.

## Competitors scanned

1. **Tarsnap `scrypt` CLI** (upstream reference implementation; man page + FORMAT spec).
   The de-facto standard and the only true format peer. Subcommands `enc`, `dec`, `info`.
   Flags: `--logN`/`-r`/`-p` (must be given together), `-M maxmem` (byte ceiling on the KDF
   working set), `-m maxmemfrac` (fraction of available RAM, ≤0.5), `-t maxtime` (CPU-second
   budget used to auto-pick parameters), `-f` (force past resource warnings), `-v` (print the
   chosen N/r/p and resource limits), `--passphrase method:arg` (tty / stdin / env var / file),
   `--version`. Defaults for logN/r/p are *not* fixed — they are benchmarked per machine.
   `scrypt info` reports the parameters and the memory needed to decrypt, without a passphrase.
2. **encrypt-decrypt.net (scrypt page)** — browser-local crypto utility that advertises scrypt
   alongside encrypt/decrypt and verifiable secret sharing. The page is mostly a shell; the
   fetch surfaced no parameter list, defaults, or documented limits, so it contributes only the
   "everything client-side, no upload" positioning, not a feature bar.
3. **cryptfile.online** — browser-local file/text encryptor. AES-256-GCM (default) or
   ChaCha20-Poly1305; PBKDF2 with 600k iterations; key sourced from a password, a generated
   random 256-bit key, or a user-supplied base64/hex key. Drag-and-drop file picker, optional
   pre-compression, key-visibility toggle, copy buttons, one-click download of the result,
   chunked processing. Stated limits: 10k characters for text mode, degraded performance past
   ~500 MB files, lost keys unrecoverable. Custom `.enc` container — **not** scrypt format.
4. **8gwifi.org file encrypt** — browser-local file encryptor using WebCrypto. PBKDF2-SHA256,
   10k iterations, random 8-byte salt; 384-bit key stream split into an AES-256 key and a CBC
   IV; output framed as `Salted__` + salt + ciphertext (OpenSSL-compatible). Password plus a
   confirmation field (min 8 chars), built-in password generator, drag-and-drop, `.enc`
   suffix on download and suffix-stripping on decrypt. Again a different container.
5. **Browserling scrypt hasher** — scrypt *KDF* page (not a container tool). Fields: password,
   salt, output size, and N / r / p labelled as CPU / memory / parallelization difficulty. No
   documented defaults, no presets, no explanatory copy. Useful only as evidence of how the
   cost parameters are normally surfaced to users (three plain numeric fields, plain-English
   labels).

Nothing else scanned actually reads or writes the scrypt container format in a browser — the
"online file encryption" field is uniformly AES-GCM/CBC with PBKDF2 and a bespoke wrapper. That
is the differentiator here: format compatibility with the `scrypt` CLI, not another `.enc` box.

## Table stakes → where each one landed

| Table stake (source) | Decision |
| --- | --- |
| `enc` / `dec` / `info` subcommands (1) | **In-model, built** — `operation` enum `encrypt`/`decrypt`/`info`. |
| Explicit `--logN` / `-r` / `-p` (1, 5) | **In-model, built** — `log_n` (1–63, default 14), `r` (1–32, default 8), `p` (1–16, default 1). |
| `-M maxmem` memory ceiling (1) | **In-model, built** — `max_memory_mib`, default 32, hard max 64 (the wasm sandbox), rendered as a slider. Over-budget parameters are refused with the exact required amount rather than trapping. |
| `info` reports parameters + memory needed, no passphrase (1) | **In-model, built** — `operation=info` parses the header, verifies the checksum, and prints logN/N/r/p/salt/estimated memory/section sizes, plus the `max_memory_mib` value that would be needed. Works on files too large to decrypt here. |
| Authenticated decryption; wrong password ≠ garbage (1, 3, 4) | **In-model, built** — header HMAC and file HMAC are both constant-time checked before any plaintext is returned; distinct error messages for wrong passphrase vs tampered body. |
| Binary in/out, not just text (3, 4) | **In-model, built** — `data_encoding` = text/hex/base64 and `output_encoding` = base64/hex; non-UTF-8 plaintext comes back in the chosen encoding. |
| Auto-detect the pasted container encoding (3) | **In-model, built** — `data_encoding=text` auto-detects hex vs base64 when decrypting or inspecting. |
| Deterministic/reproducible output for tests (1) | **In-model, built** — optional 64-hex-char `salt` override; empty means 32 fresh random bytes per run. |
| Copy / download the result (3, 4) | **In-model, already provided** — the generator gives `format = "text"` pages a copy control and a Download link; no per-tool work needed. |
| Preset / example buttons (3, 5) | **In-model, built** — four `[[example]]` chips: fresh-salt encrypt, reproducible hex vector, decrypt that vector, inspect without a passphrase. |
| Plain-English labels for the cost parameters (5) | **In-model, built** — `[input.labels]` on both enums and spelled-out field labels ("logN (CPU/memory cost)", "r (block size)", "p (parallelization)"). |
| Stated limits on the page (3) | **In-model, built** — 4 MiB decoded-input cap, 64 MiB sandbox ceiling and the resulting logN≈16 practical limit, version-0-only, empty-plaintext behaviour, all in `content.md`. |
| Password strength / generator (4) | **Considered, rejected** — a generator belongs in the existing password-generator/password-entropy tools; duplicating it here bloats the schema of a format tool with a param chat and the CLI would have to carry. |
| Password confirmation field (4) | **Considered, rejected** — a confirm box cannot be a descriptor param (chat and CLI have no use for it) and adding a page-only field would break the single-source descriptor rule. |

## Out-of-model (listed, not built)

- **`-t maxtime` / `-m maxmemfrac` auto-tuning (1).** Both pick parameters by benchmarking the
  host's CPU and free RAM. Neither quantity is observable from a wasm sandbox, and both make
  the output machine-dependent, which breaks determinism across chat/CLI/page. The FAQ states
  this explicitly and points at `max_memory_mib` as the `-M` analogue.
- **`--passphrase method:arg` (tty/stdin/env/file) (1).** Pure CLI plumbing for how the secret
  reaches the process; the gizza CLI takes `password=` and chat passes a string. Nothing to
  build in the block.
- **`-f` force (1).** Its job is to override the auto-tuner's resource warnings. With explicit
  parameters and an explicit memory ceiling there is no warning to override — raising
  `max_memory_mib` *is* the override, up to the sandbox limit.
- **File upload / drag-and-drop and multi-hundred-MB streaming (3, 4).** The page file-input
  source is wired to the ffmpeg runtime; a pure block's page takes fields only. Large files
  also exceed the 4 MiB paste cap by design. The CLI is the path for real files.
- **Pre-compression before encryption (3).** Changes the plaintext, so the container would no
  longer round-trip through `scrypt dec` to the user's original bytes — it would silently
  break format compatibility, which is the entire point of this tool.
- **Alternative ciphers/KDFs, key files, random-key mode (3, 4).** Version 0 of the format
  fixes scrypt + AES-256-CTR + HMAC-SHA256 and derives both keys from a passphrase. Any option
  here would produce a blob that looks like a scrypt file but no compatible implementation
  could open.

## UX decisions worth recording

- **Slider only on `max_memory_mib`.** `kind = "slider"` fits a bounded, independently
  meaningful range. `log_n`, `r` and `p` are *jointly* constrained (`128 * 2^logN * r` must fit
  the memory budget), so a slider sweeping logN to 63 or r to 32 would advertise combinations
  that can never run. They stay number boxes with the reference defaults as placeholders, and
  the preset chips cover the useful combinations instead.
- **`log_n` keeps its full 1–63 range** because that is the range the format defines and
  `operation=info` must be able to describe such files honestly. Encryption above roughly
  logN 16 (at r=8) is refused by the memory gate with a message naming the required MiB, which
  is more informative than silently narrowing the range.

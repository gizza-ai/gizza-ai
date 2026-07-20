# encrypted-zip — competitor analysis (2026-07-19)

Scan done BEFORE implementation (create-next-tool step: competitor scan). One WebSearch for
"create password protected zip online AES encrypted zip file tool"; top 3 reachable real tools
skimmed. WC ZIP (wczip.com) returned HTTP 403 and was replaced with PrivConvert per the
"replace unreachable competitors" rule. All notes are paraphrased — no competitor copy,
branding, or trademarks reproduced.

## Competitors skimmed

1. **ezyZip** (ezyzip.com/zip-files-with-password.html) — browser-local WASM zip-with-password.
   AES encryption (variant unspecified), adjustable compression level via a dropdown, drag-and-drop
   multi-file add, password entry + confirm flow, local download. Ships a *separate* extractor for
   password-protected zips. Emphasizes that files never leave the browser.
2. **ConvertResize** (convertresize.com/other/password-protected-zip.html) — client-side creator
   with an explicit two-mode choice: legacy ZipCrypto ("compatible": opens natively in Windows
   Explorer / macOS / iOS) vs AES-256 ("maximum security": needs 7-Zip/The Unarchiver class tools).
   Dual password fields with confirmation + a live strength meter. Notes that file NAMES stay
   visible inside the archive under both modes. Creator only — no extraction.
3. **PrivConvert** (privconvert.com/tools/protect-zip/) — single-mode AES-256 protect tool,
   password field + one action button, 250 MB stated limit, in-memory/zero-storage processing.
   Extraction shipped as a separate "unlock zip" sibling tool.

## Table-stakes → decision (every item lands in the descriptor or the out-of-model list)

| Table-stake (seen at) | Tag | Where it landed |
| --- | --- | --- |
| AES-256 encryption, default (all 3) | in-model | `encryption=aes256` default (WinZip AE-2 via the `zip` crate `aes-crypto` feature) |
| AES key-strength choice (ConvertResize mode choice; WinZip-style tools offer 128/256) | in-model | `encryption` enum `aes256\|aes128` |
| Extraction of password-protected zips (ezyZip + PrivConvert sibling tools; backlog row) | in-model | `mode=extract` — reads AES-256/192/128 **and** legacy ZipCrypto archives (method auto-detected) |
| Compression level control (ezyZip dropdown) | in-model | `level` integer 1–9, default 6 (deflate; mirrors `file-compressor`'s level contract) |
| Clear wrong-password error (all — implied by the password-confirm flows) | in-model | AES password verifier + ZipCrypto check byte → explicit "wrong password" error, not a decode dump |
| Stated size limits (PrivConvert 250 MB) | in-model | caps in the tool description: 8 MiB/input file, 32 MiB output zip, 32 MiB archive in, 8 MiB inlined content on extract |
| Local/in-memory processing, nothing uploaded (all 3) | in-model | inherent: wasm block runs locally; stated in the description |
| Compatibility guidance (ConvertResize's headline) | in-model (copy) | description says AES zips need 7-Zip/WinZip/WinRAR/The Unarchiver-class tools, not Windows Explorer |
| **ZipCrypto WRITE ("compatible mode", ConvertResize)** | **out-of-model** | Spiked: `zip` 8.6's ZipCrypto writer (`with_deprecated_encryption`) is `pub(crate)` — no public API; only AES write is public. Reading ZipCrypto IS supported. Also defensible on merit: ZipCrypto is cryptographically broken, so we decrypt legacy archives but never create new ones. Listed, not built. |
| Password confirm field + strength meter (ezyZip, ConvertResize) | out-of-model | page-form UX; this tool is chat+CLI only (file-input/binary-output family has no page, like create-zip/unzip/7z-extract). `password-entropy` / `weak-password-detector` blocks already cover strength checking. |
| Drag-and-drop multi-file upload, Dropbox/cloud pickers (ezyZip) | out-of-model | no page in this family; chat attachments / CLI URLs are the input surface |

## Design conclusions

- One tool, two modes (`mode=pack|extract`), mirroring the backlog row; `files` source_list serves
  both (pack: N files; extract: exactly 1 = the archive). Sibling-family invariants copied from
  `create-zip` (pack output: base64 `application/zip` envelope, duplicate entry names made unique)
  and `unzip` (extract output: flat JSON entries with inline text/base64 content under a budget).
- Decompression-bomb hardening per the 2026-07-17 sweep (#217): `.take()` + declared-size guard
  per entry, 8 MiB inline-content budget, caps below the 64 MiB wasm sandbox.
- Not a dup: `create-zip`/`unzip` are deliberately deflate-only (no password support),
  `encrypt-file`/`text-encrypt` produce a proprietary AES-GCM blob (not zip-interoperable),
  `7z-extract` handles 7z (different container). This tool's output opens in standard zip tools.

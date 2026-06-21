# extract-decode-base64 — competitor analysis & differentiation

**Tool:** `gizza-ai/extract-decode-base64` — scan text for embedded Base64 blobs,
decode them, and show decoded text or a file-type + hex preview.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| base64decode.org, cryptii, CyberChef | Web | Decode **one** blob you paste in — you must first find and isolate each blob yourself. CyberChef can do more but needs recipe-building; most upload-or-run-remote. |
| `base64 -d` (coreutils) | CLI | Decodes a whole stream, not embedded blobs inside prose; errors on non-Base64 surrounding text. |
| `gizza multi-encoder` (our own) | tool | Explicit encode/decode of a known string — not detection within arbitrary text. |
| Manual regex + decode scripts | DIY | Have to handle alphabet variants, padding, false positives, and binary vs text yourself. |

## How gizza's tool is better / different

1. **Finds the blobs for you.** Scans arbitrary text (logs, JSON, headers, data
   URIs) and decodes *every* embedded Base64 run — no manual copy-paste of each
   token. This is the key difference from plain decoders.
2. **Text vs binary, automatically.** Printable results come back as decoded
   text; binary results are labelled with a detected **file type** (reusing
   gizza's magic-byte sniffer) plus a hex preview — so you instantly know a blob
   is a PNG, PDF, gzip, etc.
3. **Both alphabets, padded or not.** Standard (`+`/`/`) and URL-safe (`-`/`_`)
   Base64, with or without `=` padding.
4. **Noise-filtered.** Random alphanumeric runs that decode to neither printable
   text nor a known file type are dropped, so you don't drown in false positives.
5. **Local + three surfaces.** Chat ("decode the base64 in this log"), CLI, and a
   zero-upload page, all one Rust core.

## Verification

CLI run on a string containing a `Basic` auth token and an embedded PNG returned
both: the token decoded to "Hello, world! This is a test." (text), and the PNG
blob was labelled `image/png` with hex preview `89504e470d0a1a0a…`.

## Scope / honest limitations

- Heuristic detection: very short blobs (<16 chars) are skipped, and a binary
  blob whose format isn't in the sniffer's table won't be reported (avoids noise).
- Doesn't recurse (decode Base64-within-Base64) — could be a future option.

## Possible future enhancements

- Emit decoded binaries as downloadable data-URIs (file-output envelope).
- Optional "aggressive" mode that reports all decodable blobs regardless of the
  printable/known-type filter.
- Recursive decode for nested encodings.

# encoded-payload-decoder — competitor analysis (2026-07-20)

Tool function: given a file, find and decode base64/hex tokens and gzip/zlib compressed
streams embedded anywhere in the bytes, unwrap nested layers, and surface hidden readable
strings + detected file types. A forensics / CTF / malware-triage "decode everything" pass.

## Competitors scanned (top 3, paraphrased — no copy/branding reproduced)

1. **CyberChef "Magic" (gchq.github.io/CyberChef)** — the reference forensic Swiss-army tool.
   Its Magic operation runs regexes for every encoding (base64, hex, gzip, …) over the data,
   records matches, and recursively suggests decode chains. Drag-and-drop file input (up to
   ~2 GB browser limit). Also scores candidates by entropy + an English-language dictionary to
   rank likely-correct decodes, and includes XOR/rotate brute-force in the Magic search. Runs
   entirely client-side.

2. **Base64 Gzip decoders (Solution Toolkit, Simon Willison's base64-gzip-decoder, JSON-to-Table
   base64-to-gzip, Openformatter GZIP)** — a family of single-purpose tools that take a base64
   string, decode to bytes, then gzip/zlib decompress to text, in one shot. Motivated by AWS
   Lambda/Kinesis logs which ship gzip+base64. Text-in / text-out, browser-only.

3. **Malware-triage decode chains (az4n6 forensics blog + mattnotmax/cyberchef-recipes)** — the
   established manual workflow: peel one layer at a time (base64 → decompress → base64 → shellcode),
   using `strings` on binary layers to surface readable IOCs (URLs, paths, commands). This is the
   real-world "nested layers" job our tool automates in a single pass.

## Table-stakes (each tagged in-model / out-of-model)

| Capability | Decision |
| --- | --- |
| Detect + decode base64 (standard AND url-safe alphabet) | IN — regex token scan, both alphabets |
| Detect + decode hex (even-length runs) | IN — regex token scan |
| Detect + decompress gzip (magic `1f 8b 08`, anywhere in file) | IN — flate2 GzDecoder from each magic offset |
| Detect + decompress zlib (magic `78 01/5e/9c/da`) | IN — flate2 ZlibDecoder; Adler-32 check kills false positives |
| Recursive / nested layers (base64 → gzip → text, etc.) | IN — `max_depth` param, BFS over decoded buffers |
| Surface readable strings from binary payloads | IN — internal `strings`-style ASCII+UTF-16 scan on each binary finding |
| File-type detection for binary payloads | IN — magic-byte sniffer (PNG/JPEG/GIF/PDF/ZIP/ELF/…) |
| Report WHERE each payload was found (offset + nesting depth) | IN — offset within parent buffer + depth |
| Minimum candidate token length (avoid noise) | IN — `min_len` param |
| Runs locally / nothing uploaded | IN — pure-Rust, file never leaves the device |
| Entropy + dictionary confidence SCORING / candidate ranking | OUT — CyberChef's Magic scoring; heuristic ranker, out of scope for a deterministic decoder. We report every valid decode, unranked. |
| XOR / bit-rotate brute-force key search | OUT — CyberChef Magic includes it; a keyspace brute-force is a different tool class (would need its own block). |
| Interactive recipe building / drag-and-drop 2 GB browser UI | OUT — this is the public toolkit repo; file arrives via url/ref (capped at 8 MiB), no page (file-input → JSON report, like `strings`/`detect-file-type`). The branded page lives in the private site repo. |
| URL/IOC extraction as a distinct list | PARTIAL — covered by surfaced strings (URLs appear as printable strings); no separate IOC regex list. |

## UX controls

Competitors are browser pages with file drag-and-drop and live recipe chips. This block is a
**file-input → JSON report** tool with NO standalone page (the pure-page runtime only wires field
inputs, not file uploads — same shape as `strings`, `detect-file-type`, `byte-entropy`, `hex-view`).
Its surfaces are chat + CLI, so slider/color/preset-chip controls do not apply. Parameters
(`min_len`, `max_depth`) are plain scalars.

## Design decisions

- Two scalar params only: `min_len` (default 20; min token length for base64/hex candidates) and
  `max_depth` (default 3; nested-decode layers). Keeps the chat/CLI surface tight.
- base64 acceptance mirrors the existing `extract-decode-base64` heuristic (printable-ratio ≥ 0.85
  OR a recognized file type) so random alphanumeric noise is not reported — but a decoded blob that
  is itself compressed/encoded is always recursed into regardless of printability.
- Decompression-bomb defense: per-stream output capped (4 MiB) and a total decode budget across the
  whole scan; findings capped at 200. Adler-32 (zlib) / CRC-32 (gzip) validation on full
  decompress is what makes the 2-byte zlib magic usable without drowning in false positives.
- Out-of-model rows above are listed, not built.

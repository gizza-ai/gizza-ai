# qr-paper-backup — competitor analysis (2026-07-26)

**Tool function:** encode a file or secret into a printable sheet of numbered QR codes for
offline paper archival (the *encode* direction; the *restore/decode* multi-image direction is
tracked separately in `docs/tool-skiplist.txt` as `qr-backup-restore`, which has no verifiable
surface on this box).

**Surface:** image-bytes output (an SVG sheet) → `build_media_envelope`, so **chat + CLI, no page**
(same shape as `qr-code-generator` / `wifi-qr-code-generator`). No Playwright spec applies.

## Competitors scanned (paraphrased — no copy/branding reused)

1. **qr-backup** (za3k, GitHub) — CLI; file → printable B/W PDF of QR codes. ~3 KB/page default,
   up to ~130 KB/page dense. Automatic compression, optional password, per-page redundancy
   (survive losing ~30 % of codes), printed restore instructions, restore via webcam/scanner or
   plain Linux tools.
2. **paperify** (alisinabh, GitHub) — splits a file into chunks, one QR per chunk, adds
   human-readable metadata to each code. Each A4 page holds up to ~2953 bytes.
3. **paperbackup** (intra2net, GitHub) — PDF of barcodes for GnuPG/SSH keys + ciphertext. Splits
   at ~140 bytes/code to stay reliably decodable, labels each barcode with a **start marker +
   sequence number**, prints the **plaintext fallback** next to the codes, and a per-line checksum
   (first hex chars of the line's digest).
4. **QRbackup** (qrbackup.github.io) — ~65 KB/A4 page; each QR carries a **preamble indicating the
   chunk index** (e.g. `9/10:`).
5. **SeedQR / seed-vault** (Bitcoin ecosystem) — single-secret (seed phrase) → QR; emphasis on
   offline generation and never touching the network.

## Table-stakes → decision

| Capability | Competitors | Decision |
|---|---|---|
| Split arbitrary data across multiple QR codes | all | **in-model** — chunk by byte size |
| Sequence header/preamble inside each QR (`i/n`) | paperbackup, QRbackup | **in-model** — `QRB1\|i\|n\|id\|b64` line |
| Whole-set integrity id / checksum | paperbackup, (SeedQR ethos) | **in-model** — SHA-256 of the full data; short id in every code |
| Human-readable caption under each code (`Part i / n`) | paperify, paperbackup | **in-model** |
| Printed plaintext fallback next to each code | paperbackup | **in-model** — `show_text` (default on) prints the code's line |
| Error-correction level choice (L/M/Q/H) | qr encoders generally | **in-model** — `error_correction` enum |
| Chunk size control (bytes per code) | paperbackup 140, paperify 2953 | **in-model** — `chunk_bytes` (50–1200, default 300) |
| Grid layout / codes per row | all (page layout) | **in-model** — `columns` (1–5, default 3) |
| Printed restore instructions on the sheet | qr-backup | **in-model** — header block spells out the format + restore steps |
| File input (not just text) | all | **in-model via encoding** — `input_encoding` = text / base64 / hex (paste a file's bytes as base64/hex; avoids the broken file-fetch/network path) |
| Printable single-file output | PDF | **in-model** — SVG (scalable, prints to PDF from any browser; one self-contained file) |
| Automatic compression | qr-backup | **out-of-model** — would break the re-typeable plaintext fallback; user can compress first |
| Encryption / password | qr-backup, seed-vault | **out-of-model** — chain gizza's `encrypt-file` / `text-encrypt` first, then back up the ciphertext |
| Erasure-coding redundancy (lose 30 % of codes) | qr-backup | **out-of-model** — needs Reed–Solomon page-level FEC; QR's own ECC (L/M/Q/H) is the in-model redundancy |
| Restore/decode direction | all | **out-of-model here** — multi-image input has no page/CLI surface on this box (see `qr-backup-restore` skiplist) |

## Format shipped (documented on the sheet + in `.describe()`)

Each QR code encodes one line: `QRB1|<index>|<total>|<id>|<base64-chunk>` where `<id>` is the
first 8 hex chars of the SHA-256 of the full data (ties the set together). Restore = collect codes
1..n in order, concatenate the base64 fields, Base64-decode. Deterministic output (no timestamps),
so it is byte-reproducible and exact-output testable.

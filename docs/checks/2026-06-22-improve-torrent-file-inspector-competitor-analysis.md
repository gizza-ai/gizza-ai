# torrent-file-inspector — competitor analysis (2026-06-22)

Tool: parse a BitTorrent `.torrent` (bencode) file and report its metainfo —
name, file list with sizes, total size, trackers, piece length/count, private
flag, the v1 info-hash (SHA-1 of the bencoded `info` dict), and the optional
comment / created-by / creation-date.

Type: **pure** (hand-rolled bencode parser + `sha1` for the info-hash), file
input via `Input::File` (url ⊕ ref). Surfaces: **chat + CLI**. No standalone
page — a file→JSON report fits neither the pure-text page nor the ffmpeg
file→media page shape (the F3 "no-page file-input" pattern, like
`detect-file-type` / `web-fetch`).

## Top competitors surveyed

1. **torrent-file-editor (GUI app, GitHub)** — full read/write GUI for the
   metainfo dict; shows the info-hash, files, trackers, piece info, comment,
   created-by, dates, plus arbitrary tree editing.
2. **WebTorrent `parse-torrent` (JS lib) / its web demos** — decodes a
   `.torrent`/magnet into `{infoHash, name, files[{name,length,path}], length,
   pieceLength, lastPieceLength, pieces, announce[], urlList, comment,
   createdBy, created, private}`.
3. **transmission-show / `transmission-edit` (CLI)** — prints name, hash, created
   by/on, comment, piece count + size, total size, privacy, the tracker tier
   list, and each file with its size.
4. **Online ".torrent file viewer / editor" sites (e.g. torrenteditor-style)** —
   browser tools that dump the bencode tree, info-hash, file table, and trackers;
   some build the magnet link.
5. **`torf` / `torrenttools` (CLI/lib)** — report info-hash (v1/v2), name, size,
   piece size + count, file tree, trackers (tiered), private flag, comment,
   creation date, created-by, source.

## Capability diff vs. this tool

| Capability | Competitors | This tool | Notes |
|---|---|---|---|
| Suggested name | yes | **yes** | `name` |
| File list + per-file size | yes | **yes** | `files[].path/.bytes`, root dir prefixed |
| File count | yes | **yes** | `file_count` |
| Total size | yes | **yes** | `total_bytes` |
| Trackers (announce + announce-list, deduped) | yes | **yes** | `trackers`, declaration order, deduped |
| Piece length | yes | **yes** | `piece_length` |
| Piece count | yes | **yes** | `piece_count` (`pieces`/20) |
| Private flag | yes | **yes** | `private` |
| Info-hash (v1, SHA-1 of bencoded `info`) | yes | **yes** | `info_hash`, verified == known Sintel btih `08ada5a7…5a10` |
| Single vs multi-file | yes | **yes** | `is_multi_file` |
| Comment / created-by / creation-date | yes | **yes** | optional fields, omitted when absent |

### Gaps deliberately not closed (out of model / out of scope)

- **Magnet-link construction** — could be added, but the v1 `info_hash` is the
  load-bearing value (`magnet:?xt=urn:btih:<info_hash>&dn=<name>`); the LLM/user
  can assemble the magnet from the returned hash + name + trackers. Left as a
  possible future enhancement rather than baked-in copy.
- **BitTorrent v2 / hybrid (`meta version 2`, `file tree`, `pieces root`)** —
  v2 info-hashes are SHA-256 over a different structure. Rare in the wild;
  reporting only the v1 hash matches the common case (transmission-show, older
  parse-torrent). Out of scope for v1 of the tool.
- **`url-list` (web seeds / GetRight)** and **DHT `nodes`** — niche metainfo
  keys; omitted to keep the response focused on the fields users actually read.
- **Editing / re-encoding** the torrent (the GUI editors) — this is a read-only
  inspector by design; mutation isn't a gizza tool shape here.
- **Magnet/`.torrent` from a URL only** — input is already url ⊕ ref via
  `Input::File`, matching every other file-input tool.

No competitor copy, branding, or trademarks were used; field names follow the
neutral BitTorrent metainfo (BEP-3) terminology.

## Verification (2026-06-22)

- `cargo test --workspace` in `blocks/torrent-file-inspector/` — 7 tests pass
  (core: single-file, multi-file + announce-list dedupe + meta, info-hash
  stability/correctness, empty/non-torrent errors, bencode primitives; block:
  chat-schema drift guard).
- `wafer build` — chat `block.wasm` validates & instantiates (517 KiB).
- `cargo run -p generator -- .` — regenerates the site cleanly; no page emitted
  for this tool (no-page F3 pattern), as intended.
- **CLI**: `gizza tool torrent-file-inspector url="https://webtorrent.io/torrents/sintel.torrent"`
  returns the correct Sintel metainfo, including the canonical info-hash
  `08ada5a7a6183aae1e09d831df6748d566095a10` (cross-checked against the public
  WebTorrent Sintel torrent).
- Page Playwright: **N/A** — no standalone page for a file→JSON tool.

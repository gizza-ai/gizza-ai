# bencode-decoder — competitor analysis (2026-06-22)

Tool: `blocks/bencode-decoder` — convert between **bencode** (the BitTorrent
serialization format) and **JSON**, both directions. Pure-Rust hand-rolled codec
(no external crate). Surfaces: chat, CLI, page.

## Top competitors surveyed

1. **chocobo1 / bencode-online (bencode.fly.dev)** — popular single-page bencode↔JSON
   web tool. Decode and encode directions; renders binary fields (e.g. torrent
   `pieces`) as escaped/hex. Client-side, no upload.
2. **lkwbr / bencode-online & various "bencode editor" gists** — decode bencode to a
   tree/JSON view; some allow editing and re-encoding.
3. **jsfiddle / npm `bencode` (themasch)** — the de-facto JS library: `bencode.decode`
   / `bencode.encode`; decodes byte strings to Buffers, encodes objects back. Powers
   most online tools.
4. **Python `bencode.py` / `bencodepy`** — CLI/library; `bdecode`/`bencode`; raises on
   non-canonical input; keeps dict key ordering canonical on encode.
5. **`transmission-show` / `aria2c --show-files` (torrent CLIs)** — not general bencode
   tools; they decode a `.torrent` to a human report. (This is gizza's separate
   `torrent-file-inspector`, not a dup of this general codec.)

## Capability diff (us vs. the field)

| Capability | Competitors | bencode-decoder | Status |
|---|---|---|---|
| Decode bencode → JSON | yes (all) | yes | ✅ at par |
| Encode JSON → bencode | yes (1,2,3,4) | yes | ✅ at par |
| Canonical dict-key sort on encode | bencodepy, bencode.py; many JS tools DON'T | **yes** (sorts by byte value) | ✅ ahead of typical web tools |
| Reject malformed input (leading zeros, `-0`, length overflow, trailing data, non-string keys) | strict libs only | **yes** | ✅ at/above par |
| Lossless binary byte strings (e.g. `pieces`) | hex/escape display; rarely round-trippable | **yes** — `{"_bencode_bytes_hex":"…"}` sentinel, decode→encode reproduces exact bytes | ✅ ahead |
| Pretty / compact JSON toggle | some | yes (`pretty`) | ✅ at par |
| Runs fully client-side / private | yes (web tools) | yes (browser wasm) + CLI + chat | ✅ at par + extra surfaces |

## Gaps considered and decisions

- **Tree/collapsible JSON viewer** (some web editors): out of model — the gizza page
  driver renders a single text output box. Not in scope; JSON text is the standard
  interchange and is copy-pasteable. Skipped (UX-only, framework-bound).
- **Edit-in-place of a decoded tree then re-encode**: same framework limitation; the
  decode→edit JSON→encode round-trip is already achievable by running the tool twice.
  Skipped.
- **`.torrent` file upload / info-hash report**: deliberately NOT added — that is the
  distinct existing tool `torrent-file-inspector` (file input + SHA-1 info-hash). This
  tool is the general text↔text codec; keeping them separate avoids duplication.
- **Boolean/null handling**: bencode has neither type; we surface a clear error rather
  than silently coercing. Matches strict libraries (bencodepy). Documented in copy.

## Conclusion

bencode-decoder is at or ahead of the surveyed competitors on the in-model
capabilities: bidirectional conversion, **canonical key sorting**, strict malformed-input
rejection, and **lossless binary round-tripping** (which most web tools lack). The only
gaps are framework-bound UX features (tree view / inline editing) that don't fit gizza's
single text-output page model. No competitor copy/branding was used. No further in-model
gaps to close.

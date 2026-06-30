## About this tool

A **magnet link** is a `magnet:?…` URI that identifies a BitTorrent download by
its content hash rather than by a `.torrent` file on a server. This tool reads
one apart into its pieces — or assembles a fresh one from the parts you supply —
entirely in your browser. Nothing you paste is uploaded.

### Parse mode

Paste a magnet link and get back every component, decoded:

- **Info hash (v1)** — the `urn:btih:` value, normalised to lower-case hex
  whether the link used 40 hex characters or 32 base32 characters, with the
  original encoding noted.
- **Info hash (v2)** — the `urn:btmh:` multihash, for BitTorrent v2 links.
- **Display name** (`dn`) — percent- and `+`-decoded back to readable text.
- **Trackers** (`tr`) — every announce URL, in order.
- **Web seeds** (`ws`) — HTTP/FTP seed URLs (BEP 19).
- **Acceptable / exact sources** (`as` / `xs`), **keywords** (`kt`), and the
  **exact length** (`xl`) in bytes, shown with a human-readable size.
- Any other parameters are listed verbatim, and indexed keys such as `tr.1` and
  `tr.2` are collapsed to their base key.

### Build mode

Switch the mode to **build**, enter an info-hash (40 hex characters, 32 base32
characters, or a full `urn:btih:…` value) and any optional display name,
trackers, web seeds and exact length, and the tool returns a ready-to-use
`magnet:?…` link with every field correctly percent-encoded.

### Privacy

Everything runs locally in WebAssembly. No magnet link, hash, or tracker URL
ever leaves your device, and there is no sign-up.
